pub mod capture;
pub mod color;
pub mod mouse_hook;
pub mod ocr;
pub mod translate;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::capture::capture_primary_monitor;
use crate::color::sample_colors;
use crate::mouse_hook::GlobalInputMonitor;
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
    pub input_monitor: Arc<GlobalInputMonitor>,
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

fn close_overlay_internal(app_handle: &AppHandle, state: &AppState) {
    state.is_live_scroll_enabled.store(false, Ordering::SeqCst);
    state.input_monitor.set_scroll_active(false);
    state.input_monitor.set_overlay_visible(false);

    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        let _ = overlay.set_ignore_cursor_events(false);
        let _ = overlay.hide();
        let _ = overlay.emit("live-scroll-status", false);
    }
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

    // Clear overlay first so previous text isn't captured in the screenshot
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        let _ = overlay.emit("screen-is-scrolling", ());
        tokio::time::sleep(Duration::from_millis(35)).await;
    }

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
        state.input_monitor.set_overlay_visible(true);
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
    state.input_monitor.set_scroll_active(enabled);
    state.input_monitor.set_overlay_visible(enabled);

    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        if enabled {
            let _ = overlay.set_ignore_cursor_events(true);
            let _ = overlay.show();
            let _ = overlay.emit("live-scroll-status", true);

            let translator = state.translator.clone();
            let target_lang = state.target_lang.lock().unwrap().clone();
            let latest_store = state.latest_translation.clone();
            let overlay_clone = overlay.clone();

            tauri::async_runtime::spawn(async move {
                let _ = overlay_clone.emit("screen-is-scrolling", ());
                tokio::time::sleep(Duration::from_millis(35)).await;

                if let Ok(result) = do_translate_screen(&translator, &target_lang).await {
                    {
                        let mut store = latest_store.lock().unwrap();
                        *store = Some(result.clone());
                    }
                    let _ = overlay_clone.emit("screen-translated-data", &result);
                }
            });
        } else {
            close_overlay_internal(&app_handle, &state);
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
fn show_overlay(
    app_handle: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        overlay.show().map_err(|e| e.to_string())?;
        overlay.set_focus().map_err(|e| e.to_string())?;
        state.input_monitor.set_overlay_visible(true);
    }
    Ok(())
}

#[tauri::command]
fn hide_overlay(
    app_handle: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    close_overlay_internal(&app_handle, &state);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let translator = Arc::new(Translator::new());
    let latest_translation = Arc::new(Mutex::new(None));
    let target_lang = Arc::new(Mutex::new("zh-TW".to_string()));
    let is_live_scroll_enabled = Arc::new(AtomicBool::new(false));

    // AppHandle holder for hook callbacks
    let app_handle_holder: Arc<Mutex<Option<AppHandle>>> = Arc::new(Mutex::new(None));
    let holder_clone1 = app_handle_holder.clone();
    let holder_clone2 = app_handle_holder.clone();
    let holder_clone3 = app_handle_holder.clone();

    let translator_for_hook = translator.clone();
    let target_lang_for_hook = target_lang.clone();
    let latest_for_hook = latest_translation.clone();

    let input_monitor = Arc::new(GlobalInputMonitor::new(
        // Callback 1: on_scroll_start (immediate)
        move || {
            if let Some(ref app) = *holder_clone1.lock().unwrap() {
                if let Some(overlay) = app.get_webview_window("overlay") {
                    let _ = overlay.emit("screen-is-scrolling", ());
                }
            }
        },
        // Callback 2: on_scroll_debounced (after 280ms pause)
        move || {
            let translator = translator_for_hook.clone();
            let lang = target_lang_for_hook.lock().unwrap().clone();
            let latest_store = latest_for_hook.clone();

            if let Some(ref app) = *holder_clone2.lock().unwrap() {
                let app_clone = app.clone();

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
        },
        // Callback 3: on_escape_pressed (GLOBAL Escape key intercepted whenever overlay is visible!)
        move || {
            if let Some(ref app) = *holder_clone3.lock().unwrap() {
                let state = app.state::<AppState>();
                close_overlay_internal(app, &state);
            }
        },
    ));

    let input_monitor_clone = input_monitor.clone();
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
                            state.input_monitor.set_scroll_active(new_state);
                            state.input_monitor.set_overlay_visible(new_state);

                            if let Some(overlay) = app_handle.get_webview_window("overlay") {
                                if new_state {
                                    let _ = overlay.set_ignore_cursor_events(true);
                                    let _ = overlay.show();
                                    let _ = overlay.emit("live-scroll-status", true);

                                    let overlay_clone = overlay.clone();
                                    tauri::async_runtime::spawn(async move {
                                        let _ = overlay_clone.emit("screen-is-scrolling", ());
                                        tokio::time::sleep(Duration::from_millis(35)).await;
                                        if let Ok(result) = do_translate_screen(&translator, &target_lang).await {
                                            {
                                                let mut store = latest_store.lock().unwrap();
                                                *store = Some(result.clone());
                                            }
                                            let _ = overlay_clone.emit("screen-translated-data", &result);
                                        }
                                    });
                                } else {
                                    close_overlay_internal(&app_handle, &state);
                                }
                            }
                        } else {
                            // Standard Snapshot Translate (Ctrl+Alt+T)
                            state.is_live_scroll_enabled.store(false, Ordering::SeqCst);
                            state.input_monitor.set_scroll_active(false);

                            let overlay_clone = app_handle.get_webview_window("overlay");
                            let input_monitor_clone = state.input_monitor.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Some(ref ov) = overlay_clone {
                                    let _ = ov.emit("screen-is-scrolling", ());
                                    tokio::time::sleep(Duration::from_millis(35)).await;
                                }
                                if let Ok(result) = do_translate_screen(&translator, &target_lang).await {
                                    {
                                        let mut store = latest_store.lock().unwrap();
                                        *store = Some(result.clone());
                                    }
                                    if let Some(ref ov) = overlay_clone {
                                        let _ = ov.set_ignore_cursor_events(false);
                                        let _ = ov.show();
                                        let _ = ov.set_focus();
                                        let _ = ov.emit("screen-translated-data", &result);
                                        let _ = ov.emit("live-scroll-status", false);
                                        input_monitor_clone.set_overlay_visible(true);
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
            input_monitor: input_monitor_clone,
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
