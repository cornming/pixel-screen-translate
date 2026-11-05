use image::{ImageFormat, RgbaImage};
use std::io::Cursor;
use xcap::Monitor;

pub struct CapturedScreen {
    pub image: RgbaImage,
    pub png_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

pub fn capture_primary_monitor() -> Result<CapturedScreen, String> {
    let monitors = Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {}", e))?;
    if monitors.is_empty() {
        return Err("No monitor found".to_string());
    }

    let primary = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .ok_or_else(|| "Primary monitor not found".to_string())?;

    let width = primary
        .width()
        .map_err(|e| format!("Failed to get monitor width: {}", e))?;
    let height = primary
        .height()
        .map_err(|e| format!("Failed to get monitor height: {}", e))?;
    let scale_factor = primary
        .scale_factor()
        .map_err(|e| format!("Failed to get monitor scale factor: {}", e))?;

    let image = primary
        .capture_image()
        .map_err(|e| format!("Failed to capture monitor: {}", e))?;

    let mut png_bytes = Vec::new();
    let mut cursor = Cursor::new(&mut png_bytes);
    image
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|e| format!("Failed to encode captured image as PNG: {}", e))?;

    Ok(CapturedScreen {
        image,
        png_bytes,
        width,
        height,
        scale_factor,
    })
}
