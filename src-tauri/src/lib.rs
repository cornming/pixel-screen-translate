pub mod capture;
pub mod color;
pub mod ocr;
pub mod translate;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::capture::capture_primary_monitor;
use crate::color::sample_colors;
use crate::ocr::{recognize_image_bytes, BoundingBox};
use crate::translate::Translator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslatedBlock {
    pub id: usize,
    pub original_text: String,
    pub translated_text: String,
    pub rect: BoundingBox,
    pub bg_color: String,
    pub text_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenTranslationResponse {
    pub screen_width: u32,
    pub screen_height: u32,
    pub scale_factor: f32,
    pub blocks: Vec<TranslatedBlock>,
}

pub struct AppState {
    pub translator: Arc<Translator>,
    pub latest_translation: Arc<Mutex<Option<ScreenTranslationResponse>>>,
    pub target_lang: Arc<Mutex<String>>,
}

pub async fn do_translate_screen(
    translator: &Translator,
    target_lang: &str,
) -> Result<ScreenTranslationResponse, String> {
    // 1. Capture primary screen
    let captured = capture_primary_monitor()?;
    let scale = if captured.scale_factor > 0.0 {
        captured.scale_factor
    } else {
        1.0
    };

    // 2. Perform Windows native OCR
    let ocr_result = recognize_image_bytes(&captured.png_bytes, None)
        .await
        .map_err(|e| format!("OCR recognition failed: {}", e))?;

    // 3. Extract text lines and batch translate
    let original_texts: Vec<String> = ocr_result
        .lines
        .iter()
        .map(|l| l.text.clone())
        .collect();

    let translated_texts = translator
        .batch_translate(&original_texts, target_lang)
        .await;

    // 4. Sample background colors and map to logical coordinates
    let mut blocks = Vec::new();
    for (idx, line) in ocr_result.lines.iter().enumerate() {
        let colors = sample_colors(&captured.image, &line.rect);
        let trans = translated_texts
            .get(idx)
            .cloned()
            .unwrap_or_else(|| line.text.clone());

        let logical_rect = BoundingBox {
            x: line.rect.x / scale,
            y: line.rect.y / scale,
            width: line.rect.width / scale,
            height: line.rect.height / scale,
        };

        blocks.push(TranslatedBlock {
            id: idx,
            original_text: line.text.clone(),
            translated_text: trans,
            rect: logical_rect,
            bg_color: colors.bg_color,
            text_color: colors.text_color,
        });
    }

    Ok(ScreenTranslationResponse {
        screen_width: (captured.width as f32 / scale) as u32,
        screen_height: (captured.height as f32 / scale) as u32,
        scale_factor: scale,
        blocks,
    })
}

#[tauri::command]
async fn translate_screen(
    target_lang: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<ScreenTranslationResponse, String> {
    let lang = target_lang.unwrap_or_else(|| {
        state.target_lang.lock().unwrap().clone()
    });
    do_translate_screen(&state.translator, &lang).await
}

#[tauri::command]
async fn trigger_screen_translation(
    target_lang: Option<String>,
    app_handle: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ScreenTranslationResponse, String> {
    let lang = target_lang.unwrap_or_else(|| {
        state.target_lang.lock().unwrap().clone()
    });

    let result = do_translate_screen(&state.translator, &lang).await?;

    // Store latest result so overlay can fetch it anytime
    {
        let mut store = state.latest_translation.lock().unwrap();
        *store = Some(result.clone());
    }

    // Show and notify overlay window
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        let _ = overlay.show();
        let _ = overlay.set_focus();
        let _ = overlay.emit("screen-translated-data", &result);
    }

    Ok(result)
}

#[tauri::command]
fn get_latest_translation(
    state: tauri::State<'_, AppState>,
) -> Option<ScreenTranslationResponse> {
    state.latest_translation.lock().unwrap().clone()
}

#[tauri::command]
fn set_target_language(
    lang: String,
    state: tauri::State<'_, AppState>,
) {
    let mut current = state.target_lang.lock().unwrap();
    *current = lang;
}

#[tauri::command]
fn show_overlay(app_handle: AppHandle) -> Result<(), String> {
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        overlay.show().map_err(|e| e.to_string())?;
        overlay.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn hide_overlay(app_handle: AppHandle) -> Result<(), String> {
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        overlay.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let translator = Arc::new(Translator::new());
    let latest_translation = Arc::new(Mutex::new(None));
    let target_lang = Arc::new(Mutex::new("zh-TW".to_string()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let state = app.state::<AppState>();
                        let app_handle = app.clone();
                        let translator = state.translator.clone();
                        let latest_store = state.latest_translation.clone();
                        let target_lang = state.target_lang.lock().unwrap().clone();

                        tauri::async_runtime::spawn(async move {
                            if let Ok(result) = do_translate_screen(&translator, &target_lang).await {
                                {
                                    let mut store = latest_store.lock().unwrap();
                                    *store = Some(result.clone());
                                }
                                if let Some(overlay) = app_handle.get_webview_window("overlay") {
                                    let _ = overlay.show();
                                    let _ = overlay.set_focus();
                                    let _ = overlay.emit("screen-translated-data", &result);
                                }
                            }
                        });
                    }
                })
                .build(),
        )
        .setup(|app| {
            // Register global shortcut Ctrl+Alt+T
            if let Err(e) = app.global_shortcut().register("Ctrl+Alt+T") {
                eprintln!("Failed to register Ctrl+Alt+T: {}", e);
            }
            Ok(())
        })
        .manage(AppState {
            translator,
            latest_translation,
            target_lang,
        })
        .invoke_handler(tauri::generate_handler![
            translate_screen,
            trigger_screen_translation,
            get_latest_translation,
            set_target_language,
            show_overlay,
            hide_overlay
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
