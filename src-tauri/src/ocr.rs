use serde::{Deserialize, Serialize};
use windows::core::{HSTRING, Result};
use windows::Globalization::Language;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

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

    Ok(OcrScanResult {
        text: full_text,
        lines: recognized_lines,
    })
}
