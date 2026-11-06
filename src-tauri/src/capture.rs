use image::RgbaImage;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
use xcap::Monitor;

pub struct CapturedScreen {
    pub image: RgbaImage,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub monitor_x: i32,
    pub monitor_y: i32,
}

pub fn get_mouse_cursor_pos() -> Option<(i32, i32)> {
    let mut pt = POINT::default();
    unsafe {
        if GetCursorPos(&mut pt).is_ok() {
            Some((pt.x, pt.y))
        } else {
            None
        }
    }
}

pub fn capture_active_monitor() -> Result<CapturedScreen, String> {
    let monitors = Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {}", e))?;
    if monitors.is_empty() {
        return Err("No monitor found".to_string());
    }

    let cursor_pos = get_mouse_cursor_pos();

    let target = if let Some((cx, cy)) = cursor_pos {
        monitors.iter().find(|m| {
            let x = m.x().unwrap_or(0);
            let y = m.y().unwrap_or(0);
            let w = m.width().unwrap_or(0) as i32;
            let h = m.height().unwrap_or(0) as i32;
            cx >= x && cx < x + w && cy >= y && cy < y + h
        }).cloned()
    } else {
        None
    };

    let selected = target
        .or_else(|| monitors.iter().find(|m| m.is_primary().unwrap_or(false)).cloned())
        .unwrap_or_else(|| monitors[0].clone());

    let monitor_x = selected.x().unwrap_or(0);
    let monitor_y = selected.y().unwrap_or(0);
    let width = selected
        .width()
        .map_err(|e| format!("Failed to get monitor width: {}", e))?;
    let height = selected
        .height()
        .map_err(|e| format!("Failed to get monitor height: {}", e))?;
    let scale_factor = selected
        .scale_factor()
        .map_err(|e| format!("Failed to get monitor scale factor: {}", e))?;

    // Zero-compression capture: directly return raw RgbaImage
    let image = selected
        .capture_image()
        .map_err(|e| format!("Failed to capture monitor: {}", e))?;

    Ok(CapturedScreen {
        image,
        width,
        height,
        scale_factor,
        monitor_x,
        monitor_y,
    })
}

pub fn capture_primary_monitor() -> Result<CapturedScreen, String> {
    capture_active_monitor()
}
