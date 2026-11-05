use image::RgbaImage;
use serde::{Deserialize, Serialize};
use crate::ocr::BoundingBox;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    pub bg_color: String,
    pub text_color: String,
}

/// Samples pixels around the border of a bounding box to determine the dominant background color
/// and calculates a contrasting text color.
pub fn sample_colors(img: &RgbaImage, rect: &BoundingBox) -> ColorScheme {
    let img_w = img.width() as i32;
    let img_h = img.height() as i32;

    let x0 = (rect.x.floor() as i32).clamp(0, img_w - 1);
    let y0 = (rect.y.floor() as i32).clamp(0, img_h - 1);
    let x1 = ((rect.x + rect.width).ceil() as i32).clamp(0, img_w - 1);
    let y1 = ((rect.y + rect.height).ceil() as i32).clamp(0, img_h - 1);

    let mut r_total: u64 = 0;
    let mut g_total: u64 = 0;
    let mut b_total: u64 = 0;
    let mut count: u64 = 0;

    let sample_padding = 2;

    // Top border (just above the box)
    let top_y = (y0 - sample_padding).clamp(0, img_h - 1);
    for x in x0..=x1 {
        let pixel = img.get_pixel(x as u32, top_y as u32);
        r_total += pixel[0] as u64;
        g_total += pixel[1] as u64;
        b_total += pixel[2] as u64;
        count += 1;
    }

    // Bottom border (just below the box)
    let bottom_y = (y1 + sample_padding).clamp(0, img_h - 1);
    for x in x0..=x1 {
        let pixel = img.get_pixel(x as u32, bottom_y as u32);
        r_total += pixel[0] as u64;
        g_total += pixel[1] as u64;
        b_total += pixel[2] as u64;
        count += 1;
    }

    // Left border (just to the left)
    let left_x = (x0 - sample_padding).clamp(0, img_w - 1);
    for y in y0..=y1 {
        let pixel = img.get_pixel(left_x as u32, y as u32);
        r_total += pixel[0] as u64;
        g_total += pixel[1] as u64;
        b_total += pixel[2] as u64;
        count += 1;
    }

    // Right border (just to the right)
    let right_x = (x1 + sample_padding).clamp(0, img_w - 1);
    for y in y0..=y1 {
        let pixel = img.get_pixel(right_x as u32, y as u32);
        r_total += pixel[0] as u64;
        g_total += pixel[1] as u64;
        b_total += pixel[2] as u64;
        count += 1;
    }

    let (r_avg, g_avg, b_avg) = if count > 0 {
        (
            (r_total / count) as u8,
            (g_total / count) as u8,
            (b_total / count) as u8,
        )
    } else {
        (255, 255, 255)
    };

    // Calculate perceived luminance: 0.299*R + 0.587*G + 0.114*B
    let luminance = 0.299 * (r_avg as f64) + 0.587 * (g_avg as f64) + 0.114 * (b_avg as f64);
    let text_color = if luminance > 130.0 {
        "#111827".to_string() // Dark gray/black for light backgrounds
    } else {
        "#F9FAFB".to_string() // Light/white for dark backgrounds
    };

    ColorScheme {
        bg_color: format!("rgba({}, {}, {}, 0.96)", r_avg, g_avg, b_avg),
        text_color,
    }
}
