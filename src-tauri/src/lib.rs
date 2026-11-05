pub mod capture;
pub mod color;
pub mod mouse_hook;
pub mod ocr;
pub mod translate;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::capture::capture_primary_monitor;
use crate::color::sample_colors;
use crate::mouse_hook::MouseWheelMonitor;
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
    pub mouse_monitor: Arc<MouseWheelMonitor>,
    pub is_live_scroll_enabled: Arc<AtomicBool>,
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

    {
        let mut store = state.latest_translation.lock().unwrap();
        *store = Some(result.clone());
    }

    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        let _ = overlay.set_ignore_cursor_events(false);
        let _ = overlay.show();
        let _ = overlay.set_focus();
        let _ = overlay.emit("screen-translated-data", &result);
    }

    Ok(result)
}

#[tauri::command]
fn set_live_scroll_mode(
    enabled: bool,
    app_handle: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    state.is_live_scroll_enabled.store(enabled, Ordering::SeqCst);
    state.mouse_monitor.set_active(enabled);

    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        if enabled {
            // In live scroll mode, allow mouse wheel and clicks to pass through to the underlying app
            let _ = overlay.set_ignore_cursor_events(true);
            let _ = overlay.show();
            let _ = overlay.emit("live-scroll-status", true);

            // Trigger an immediate translation on enable
            let translator = state.translator.clone();
            let target_lang = state.target_lang.lock().unwrap().clone();
            let latest_store = state.latest_translation.clone();
            let overlay_clone = overlay.clone();

            tauri::async_runtime::spawn(async move {
                if let Ok(result) = do_translate_screen(&translator, &target_lang).await {
                    {
                        let mut store = latest_store.lock().unwrap();
                        *store = Some(result.clone());
                    }
                    let _ = overlay_clone.emit("screen-translated-data", &result);
                }
            });
        } else {
            let _ = overlay.set_ignore_cursor_events(false);
            let _ = overlay.emit("live-scroll-status", false);
        }
    }

    Ok(enabled)
}

#[tauri::command]
fn is_live_scroll_active(
    state: tauri::State<'_, AppState>,
) -> bool {
    state.is_live_scroll_enabled.load(Ordering::SeqCst)
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
fn hide_overlay(
    app_handle: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.is_live_scroll_enabled.store(false, Ordering::SeqCst);
    state.mouse_monitor.set_active(false);

    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        let _ = overlay.set_ignore_cursor_events(false);
        overlay.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let translator = Arc::new(Translator::new());
    let latest_translation = Arc::new(Mutex::new(None));
    let target_lang = Arc::new(Mutex::new("zh-TW".to_string()));
    let is_live_scroll_enabled = Arc::new(AtomicBool::new(false));

    // Create Mouse Wheel Monitor with debounced callback
    let translator_for_hook = translator.clone();
    let target_lang_for_hook = target_lang.clone();
    let latest_for_hook = latest_translation.clone();

    // AppHandle holder for hook callback
    let app_handle_holder: Arc<Mutex<Option<AppHandle>>> = Arc::new(Mutex::new(None));
    let holder_clone = app_handle_holder.clone();

    let mouse_monitor = Arc::new(MouseWheelMonitor::new(move || {
        let translator = translator_for_hook.clone();
        let lang = target_lang_for_hook.lock().unwrap().clone();
        let latest_store = latest_for_hook.clone();

        if let Some(ref app) = *holder_clone.lock().unwrap() {
            let app_clone = app.clone();
            // First emit scrolling start so UI can fade out obsolete blocks
            if let Some(overlay) = app_clone.get_webview_window("overlay") {
                let _ = overlay.emit("screen-is-scrolling", ());
            }

            tauri::async_runtime::spawn(async move {
                if let Ok(result) = do_translate_screen(&translator, &lang).await {
                    {
                        let mut store = latest_store.lock().unwrap();
                        *store = Some(result.clone());
                    }
                    if let Some(overlay) = app_clone.get_webview_window("overlay") {
                        let _ = overlay.emit("screen-translated-data", &result);
                    }
                }
            });
        }
    }));

    let mouse_monitor_clone = mouse_monitor.clone();
    let is_live_scroll_clone = is_live_scroll_enabled.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let state = app.state::<AppState>();
                        let app_handle = app.clone();
                        let translator = state.translator.clone();
                        let latest_store = state.latest_translation.clone();
                        let target_lang = state.target_lang.lock().unwrap().clone();
                        let shortcut_str = shortcut.to_string().to_lowercase();

                        if shortcut_str.contains("w") {
                            // Toggle Live Scroll Follow Mode
                            let is_active = state.is_live_scroll_enabled.load(Ordering::SeqCst);
                            let new_state = !is_active;
                            state.is_live_scroll_enabled.store(new_state, Ordering::SeqCst);
                            state.mouse_monitor.set_active(new_state);

                            if let Some(overlay) = app_handle.get_webview_window("overlay") {
                                let _ = overlay.set_ignore_cursor_events(new_state);
                                let _ = overlay.show();
                                let _ = overlay.emit("live-scroll-status", new_state);
                            }

                            if new_state {
                                // Trigger immediate scan
                                tauri::async_runtime::spawn(async move {
                                    if let Ok(result) = do_translate_screen(&translator, &target_lang).await {
                                        {
                                            let mut store = latest_store.lock().unwrap();
                                            *store = Some(result.clone());
                                        }
                                        if let Some(overlay) = app_handle.get_webview_window("overlay") {
                                            let _ = overlay.emit("screen-translated-data", &result);
                                        }
                                    }
                                });
                            }
                        } else {
                            // Standard Snapshot Translate (Ctrl+Alt+T)
                            state.is_live_scroll_enabled.store(false, Ordering::SeqCst);
                            state.mouse_monitor.set_active(false);

                            tauri::async_runtime::spawn(async move {
                                if let Ok(result) = do_translate_screen(&translator, &target_lang).await {
                                    {
                                        let mut store = latest_store.lock().unwrap();
                                        *store = Some(result.clone());
                                    }
                                    if let Some(overlay) = app_handle.get_webview_window("overlay") {
                                        let _ = overlay.set_ignore_cursor_events(false);
                                        let _ = overlay.show();
                                        let _ = overlay.set_focus();
                                        let _ = overlay.emit("screen-translated-data", &result);
                                        let _ = overlay.emit("live-scroll-status", false);
                                    }
                                }
                            });
                        }
                    }
                })
                .build(),
        )
        .setup(move |app| {
            *app_handle_holder.lock().unwrap() = Some(app.handle().clone());

            // Register global shortcut Ctrl+Alt+T
            if let Err(e) = app.global_shortcut().register("Ctrl+Alt+T") {
                eprintln!("Failed to register Ctrl+Alt+T: {}", e);
            }
            // Register global shortcut Ctrl+Alt+W (Live Wheel Follow mode)
            if let Err(e) = app.global_shortcut().register("Ctrl+Alt+W") {
                eprintln!("Failed to register Ctrl+Alt+W: {}", e);
            }
            Ok(())
        })
        .manage(AppState {
            translator,
            latest_translation,
            target_lang,
            mouse_monitor: mouse_monitor_clone,
            is_live_scroll_enabled: is_live_scroll_clone,
        })
        .invoke_handler(tauri::generate_handler![
            translate_screen,
            trigger_screen_translation,
            set_live_scroll_mode,
            is_live_scroll_active,
            get_latest_translation,
            set_target_language,
            show_overlay,
            hide_overlay
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
