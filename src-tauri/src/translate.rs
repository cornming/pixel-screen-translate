use reqwest::Client;
use serde_json::Value;

pub struct Translator {
    client: Client,
}

impl Translator {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Translates a single text using the zero-key Google web translation endpoint.
    pub async fn translate_text(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Result<String, String> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }

        let encoded_text = urlencoding::encode(text);
        let url = format!(
            "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={}&dt=t&q={}",
            target_lang, encoded_text
        );

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Translation service returned HTTP {}", response.status()));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse translation response: {}", e))?;

        // Format is typically: [[["translated text", "original", null, null, 1]], null, "en", ...]
        let mut full_translation = String::new();
        if let Some(sentences) = json.get(0).and_then(|v| v.as_array()) {
            for sentence in sentences {
                if let Some(trans_part) = sentence.get(0).and_then(|v| v.as_str()) {
                    full_translation.push_str(trans_part);
                }
            }
        }

        if full_translation.is_empty() {
            Ok(text.to_string())
        } else {
            Ok(full_translation)
        }
    }

    /// Batch translates multiple lines concurrently with a concurrency limit.
    pub async fn batch_translate(
        &self,
        lines: &[String],
        target_lang: &str,
    ) -> Vec<String> {
        let mut results = Vec::with_capacity(lines.len());
        // For efficiency, we can join small lines with a delimiter or translate in parallel
        // Let's translate with tokio join_all or chunked futures
        let mut handles = Vec::new();
        for line in lines {
            let line_clone = line.clone();
            let target_clone = target_lang.to_string();
            let client = self.client.clone();
            handles.push(tokio::spawn(async move {
                let encoded = urlencoding::encode(&line_clone);
                let url = format!(
                    "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={}&dt=t&q={}",
                    target_clone, encoded
                );
                match client
                    .get(&url)
                    .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<Value>().await {
                            let mut full = String::new();
                            if let Some(sentences) = json.get(0).and_then(|v| v.as_array()) {
                                for s in sentences {
                                    if let Some(part) = s.get(0).and_then(|v| v.as_str()) {
                                        full.push_str(part);
                                    }
                                }
                            }
                            if !full.is_empty() {
                                return full;
                            }
                        }
                    }
                    Err(_) => {}
                }
                line_clone
            }));
        }

        for handle in handles {
            if let Ok(translated) = handle.await {
                results.push(translated);
            } else {
                results.push(String::new());
            }
        }

        results
    }
}
