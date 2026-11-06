use serde::{Deserialize, Serialize};
use windows::core::{HSTRING, Result};
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, BitmapDecoder, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

pub fn get_available_ocr_languages() -> Vec<String> {
    let mut list = Vec::new();
    if let Ok(languages) = OcrEngine::AvailableRecognizerLanguages() {
        if let Ok(size) = languages.Size() {
            for i in 0..size {
                if let Ok(lang) = languages.GetAt(i) {
                    if let Ok(tag) = lang.LanguageTag() {
                        list.push(tag.to_string());
                    }
                }
            }
        }
    }
    list
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognizedWord {
    pub text: String,
    pub rect: BoundingBox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognizedLine {
    pub text: String,
    pub rect: BoundingBox,
    pub words: Vec<RecognizedWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrScanResult {
    pub text: String,
    pub lines: Vec<RecognizedLine>,
}

/// Intelligently merges lines that belong to the same sentence/paragraph.
pub fn merge_lines_into_paragraphs(lines: Vec<RecognizedLine>) -> Vec<RecognizedLine> {
    if lines.is_empty() {
        return lines;
    }

    let mut merged: Vec<RecognizedLine> = Vec::new();

    for next_line in lines {
        if next_line.text.trim().is_empty() {
            continue;
        }

        if let Some(prev) = merged.last_mut() {
            let prev_text = prev.text.trim();
            // Check if prev line ends with strong sentence terminator
            let ends_with_terminator = prev_text.ends_with('.')
                || prev_text.ends_with('?')
                || prev_text.ends_with('!')
                || prev_text.ends_with('。')
                || prev_text.ends_with('！')
                || prev_text.ends_with('？')
                || prev_text.ends_with(':')
                || prev_text.ends_with('：');

            let vertical_gap = next_line.rect.y - (prev.rect.y + prev.rect.height);
            let avg_height = (prev.rect.height + next_line.rect.height) / 2.0;
            let within_line_spacing = vertical_gap >= -5.0 && vertical_gap <= avg_height * 1.35;

            // Check horizontal alignment / overlap
            let x_overlap = (prev.rect.x + prev.rect.width).min(next_line.rect.x + next_line.rect.width)
                - prev.rect.x.max(next_line.rect.x);
            let similar_left_margin = (prev.rect.x - next_line.rect.x).abs() <= 50.0;
            let horizontally_aligned = x_overlap > 10.0 || similar_left_margin;

            // Check line height similarity (avoid merging title with small body text)
            let height_ratio = prev.rect.height / next_line.rect.height.max(1.0);
            let height_similar = height_ratio >= 0.6 && height_ratio <= 1.6;

            if !ends_with_terminator && within_line_spacing && horizontally_aligned && height_similar {
                // Merge next_line into prev!
                let is_hyphenated = prev.text.ends_with('-');
                if is_hyphenated {
                    let mut trimmed = prev.text.trim_end_matches('-').to_string();
                    trimmed.push_str(next_line.text.trim());
                    prev.text = trimmed;
                } else {
                    let prev_last_char = prev.text.chars().last().unwrap_or(' ');
                    let next_first_char = next_line.text.chars().next().unwrap_or(' ');
                    if prev_last_char.is_alphanumeric() && next_first_char.is_alphanumeric() && prev_last_char.is_ascii() {
                        prev.text = format!("{} {}", prev.text.trim(), next_line.text.trim());
                    } else {
                        prev.text = format!("{}{}", prev.text.trim(), next_line.text.trim());
                    }
                }

                // Expand bounding box
                let min_x = prev.rect.x.min(next_line.rect.x);
                let min_y = prev.rect.y.min(next_line.rect.y);
                let max_x = (prev.rect.x + prev.rect.width).max(next_line.rect.x + next_line.rect.width);
                let max_y = (prev.rect.y + prev.rect.height).max(next_line.rect.y + next_line.rect.height);

                prev.rect = BoundingBox {
                    x: min_x,
                    y: min_y,
                    width: (max_x - min_x).max(1.0),
                    height: (max_y - min_y).max(1.0),
                };

                prev.words.extend(next_line.words);
                continue;
            }
        }

        merged.push(next_line);
    }

    merged
}

pub async fn recognize_rgba_image(
    rgba_bytes: &[u8],
    width: u32,
    height: u32,
    lang_tag: Option<&str>,
) -> Result<OcrScanResult> {
    // 1. Create or select OCR Engine
    let engine = if let Some(tag) = lang_tag {
        let lang = Language::CreateLanguage(&HSTRING::from(tag))?;
        if OcrEngine::IsLanguageSupported(&lang)? {
            OcrEngine::TryCreateFromLanguage(&lang)?
        } else {
            OcrEngine::TryCreateFromUserProfileLanguages()?
        }
    } else {
        OcrEngine::TryCreateFromUserProfileLanguages()?
    };

    // 2. Direct in-memory RGBA to BGRA swap (sub-millisecond)
    let mut bgra_bytes = rgba_bytes.to_vec();
    for chunk in bgra_bytes.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    // 3. Create SoftwareBitmap directly from buffer in an isolated scope
    // so IBuffer and DataWriter are dropped before the .await call
    let bitmap = {
        let writer = DataWriter::new()?;
        writer.WriteBytes(&bgra_bytes)?;
        let buffer = writer.DetachBuffer()?;
        SoftwareBitmap::CreateCopyFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            width as i32,
            height as i32,
        )?
    };
    drop(bgra_bytes);

    // 5. Perform OCR
    let ocr_result = engine.RecognizeAsync(&bitmap)?.await?;
    let full_text = ocr_result.Text()?.to_string();

    let mut recognized_lines = Vec::new();
    let lines_view = ocr_result.Lines()?;
    let line_count = lines_view.Size()?;

    for i in 0..line_count {
        let line = lines_view.GetAt(i)?;
        let line_text = line.Text()?.to_string();
        let words_view = line.Words()?;
        let word_count = words_view.Size()?;

        let mut words = Vec::new();
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for j in 0..word_count {
            let word = words_view.GetAt(j)?;
            let word_text = word.Text()?.to_string();
            let rect = word.BoundingRect()?;

            if rect.X < min_x { min_x = rect.X; }
            if rect.Y < min_y { min_y = rect.Y; }
            if rect.X + rect.Width > max_x { max_x = rect.X + rect.Width; }
            if rect.Y + rect.Height > max_y { max_y = rect.Y + rect.Height; }

            words.push(RecognizedWord {
                text: word_text,
                rect: BoundingBox {
                    x: rect.X,
                    y: rect.Y,
                    width: rect.Width,
                    height: rect.Height,
                },
            });
        }

        if !words.is_empty() {
            recognized_lines.push(RecognizedLine {
                text: line_text,
                rect: BoundingBox {
                    x: min_x,
                    y: min_y,
                    width: (max_x - min_x).max(1.0),
                    height: (max_y - min_y).max(1.0),
                },
                words,
            });
        }
    }

    // Merge broken lines into complete natural paragraphs
    let merged_lines = merge_lines_into_paragraphs(recognized_lines);

    Ok(OcrScanResult {
        text: full_text,
        lines: merged_lines,
    })
}

pub async fn recognize_image_bytes(image_bytes: &[u8], lang_tag: Option<&str>) -> Result<OcrScanResult> {
    // 1. Create or select OCR Engine
    let engine = if let Some(tag) = lang_tag {
        let lang = Language::CreateLanguage(&HSTRING::from(tag))?;
        if OcrEngine::IsLanguageSupported(&lang)? {
            OcrEngine::TryCreateFromLanguage(&lang)?
        } else {
            OcrEngine::TryCreateFromUserProfileLanguages()?
        }
    } else {
        OcrEngine::TryCreateFromUserProfileLanguages()?
    };

    // 2. Load image into InMemoryRandomAccessStream
    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream)?;
    writer.WriteBytes(image_bytes)?;
    writer.StoreAsync()?.await?;
    writer.DetachStream()?;
    stream.Seek(0)?;

    // 3. Decode into SoftwareBitmap
    let decoder = BitmapDecoder::CreateAsync(&stream)?.await?;
    let bitmap = decoder.GetSoftwareBitmapAsync()?.await?;

    // 4. Perform OCR
    let ocr_result = engine.RecognizeAsync(&bitmap)?.await?;
    let full_text = ocr_result.Text()?.to_string();

    let mut recognized_lines = Vec::new();
    let lines_view = ocr_result.Lines()?;
    let line_count = lines_view.Size()?;

    for i in 0..line_count {
        let line = lines_view.GetAt(i)?;
        let line_text = line.Text()?.to_string();
        let words_view = line.Words()?;
        let word_count = words_view.Size()?;

        let mut words = Vec::new();
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for j in 0..word_count {
            let word = words_view.GetAt(j)?;
            let word_text = word.Text()?.to_string();
            let rect = word.BoundingRect()?;

            if rect.X < min_x { min_x = rect.X; }
            if rect.Y < min_y { min_y = rect.Y; }
            if rect.X + rect.Width > max_x { max_x = rect.X + rect.Width; }
            if rect.Y + rect.Height > max_y { max_y = rect.Y + rect.Height; }

            words.push(RecognizedWord {
                text: word_text,
                rect: BoundingBox {
                    x: rect.X,
                    y: rect.Y,
                    width: rect.Width,
                    height: rect.Height,
                },
            });
        }

        let line_rect = if word_count > 0 {
            BoundingBox {
                x: min_x,
                y: min_y,
                width: (max_x - min_x).max(1.0),
                height: (max_y - min_y).max(1.0),
            }
        } else {
            BoundingBox { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }
        };

        recognized_lines.push(RecognizedLine {
            text: line_text,
            rect: line_rect,
            words,
        });
    }

    // Merge multi-line broken sentences into complete natural paragraphs
    let paragraph_lines = merge_lines_into_paragraphs(recognized_lines);

    Ok(OcrScanResult {
        text: full_text,
        lines: paragraph_lines,
    })
}
