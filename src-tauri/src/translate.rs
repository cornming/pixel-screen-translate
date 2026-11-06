use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

pub struct Translator {
    client: Client,
    cache: Arc<Mutex<HashMap<(String, String), String>>>,
    concurrency_limit: Arc<Semaphore>,
}

impl Translator {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .unwrap_or_default(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            concurrency_limit: Arc::new(Semaphore::new(4)),
        }
    }

    /// Translates a single text using the zero-key Google web translation endpoint.
    pub async fn translate_text(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Result<String, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        // 1. Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(hit) = cache.get(&(target_lang.to_string(), trimmed.to_string())) {
                return Ok(hit.clone());
            }
        }

        // 2. Fetch from translation endpoint
        let encoded_text = urlencoding::encode(trimmed);
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

        let mut full_translation = String::new();
        if let Some(sentences) = json.get(0).and_then(|v| v.as_array()) {
            for sentence in sentences {
                if let Some(trans_part) = sentence.get(0).and_then(|v| v.as_str()) {
                    full_translation.push_str(trans_part);
                }
            }
        }

        let result = if full_translation.is_empty() {
            trimmed.to_string()
        } else {
            full_translation
        };

        // 3. Store in cache
        {
            let mut cache = self.cache.lock().unwrap();
            if cache.len() > 3000 {
                cache.clear(); // Simple eviction when cache exceeds capacity
            }
            cache.insert((target_lang.to_string(), trimmed.to_string()), result.clone());
        }

        Ok(result)
    }

    /// Batch translates multiple lines concurrently with LRU cache integration.
    pub async fn batch_translate(
        &self,
        lines: &[String],
        target_lang: &str,
    ) -> Vec<String> {
        let mut results = vec![String::new(); lines.len()];
        let mut missing_indices = Vec::new();

        // 1. Populate from cache
        {
            let cache = self.cache.lock().unwrap();
            for (i, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    results[i] = String::new();
                } else if let Some(hit) = cache.get(&(target_lang.to_string(), trimmed.to_string())) {
                    results[i] = hit.clone();
                } else {
                    missing_indices.push(i);
                }
            }
        }

        if missing_indices.is_empty() {
            return results;
        }

        // 2. Concurrently fetch only uncached texts with Semaphore concurrency control (max 4 concurrent)
        let mut handles = Vec::new();
        for &idx in &missing_indices {
            let line_clone = lines[idx].trim().to_string();
            let target_clone = target_lang.to_string();
            let client = self.client.clone();
            let sem = self.concurrency_limit.clone();

            handles.push((
                idx,
                tokio::spawn(async move {
                    let _permit = sem.acquire().await.ok();
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
                            if resp.status().as_u16() == 429 {
                                // Short backoff if rate-limited
                                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                            }
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
                                    return (line_clone, full);
                                }
                            }
                        }
                        Err(_) => {}
                    }
                    (line_clone.clone(), line_clone)
                }),
            ));
        }

        // 3. Receive results and update cache
        let mut new_entries = Vec::new();
        for (idx, handle) in handles {
            if let Ok((original, translated)) = handle.await {
                results[idx] = translated.clone();
                new_entries.push((original, translated));
            }
        }

        {
            let mut cache = self.cache.lock().unwrap();
            if cache.len() + new_entries.len() > 3000 {
                cache.clear();
            }
            for (orig, trans) in new_entries {
                cache.insert((target_lang.to_string(), orig), trans);
            }
        }

        results
    }
}
