use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;
use super::prompt::{fallback_classify_code_intent, CodeIntent};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct LlmClient {
    endpoint_url: String,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl LlmClient {
    pub fn new(endpoint_url: String, model: String, api_key: Option<String>) -> Self {
        Self {
            endpoint_url,
            model,
            api_key,
            client: reqwest::Client::new(),
        }
    }

    fn build_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(ref key) = self.api_key {
            let auth_val = format!("Bearer {key}");
            let mut val = HeaderValue::from_str(&auth_val)?;
            val.set_sensitive(true);
            headers.insert(AUTHORIZATION, val);
        }
        Ok(headers)
    }

    pub async fn test_connection(&self) -> Result<String> {
        let headers = self.build_headers()?;
        let payload = json!({
            "model": self.model,
            "messages": [
                {"role": "user", "content": "Say OK if you can read this."}
            ],
            "max_tokens": 10
        });

        let resp = self
            .client
            .post(&self.endpoint_url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("Failed to connect to LLM server at {}", self.endpoint_url))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("LLM server returned error status {status}: {body}");
        }

        let body: serde_json::Value = resp.json().await?;
        let content = body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        Ok(content)
    }

    pub async fn classify_code_intent(&self, query: &str) -> Result<CodeIntent> {
        let headers = self.build_headers()?;
        let system_prompt = "\
You are a classification tool. Your only job is to categorize user queries as either CODE (if the user asks for code to be written, generated, implemented, refactored, or demonstrated, including questions like 'how would you write...', 'can you implement...') or CHAT (if the user is greeting, chatting, asking general non-code questions, or asking what you can do). Do not write code. Reply with ONLY the single word 'CODE' or 'CHAT'.";

        let messages = json!([
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": "hello there, first time here"},
            {"role": "assistant", "content": "CHAT"},
            {"role": "user", "content": "Write a python function to parse csv files"},
            {"role": "assistant", "content": "CODE"},
            {"role": "user", "content": "who created you and what can you do?"},
            {"role": "assistant", "content": "CHAT"},
            {"role": "user", "content": "how would you implement a binary search?"},
            {"role": "assistant", "content": "CODE"},
            {"role": "user", "content": query}
        ]);

        let payload = json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
            "temperature": 0.0,
            "max_tokens": 4
        });

        let resp = self
            .client
            .post(&self.endpoint_url)
            .headers(headers)
            .json(&payload)
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(_) => return Ok(fallback_classify_code_intent(query)),
        };

        if !resp.status().is_success() {
            return Ok(fallback_classify_code_intent(query));
        }

        let body: serde_json::Value = match resp.json().await {
            Ok(b) => b,
            Err(_) => return Ok(fallback_classify_code_intent(query)),
        };

        let raw = body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_uppercase();

        if raw.contains("CODE") || raw.starts_with("```") || raw.contains("YES") {
            Ok(CodeIntent::Code)
        } else {
            Ok(CodeIntent::Chat)
        }
    }

    pub async fn stream_chat<F>(&self, messages: &[ChatMessage], mut on_token: F) -> Result<String>
    where
        F: FnMut(&str) -> Result<()>,
    {
        let headers = self.build_headers()?;
        let payload = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "temperature": 0.2,
            "max_tokens": 1536
        });

        let resp = self
            .client
            .post(&self.endpoint_url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("Failed to send request to LLM server at {}", self.endpoint_url))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("LLM server returned status {status}: {body}");
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut full_response = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.context("Error while streaming bytes from LLM")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer.drain(..=newline_pos);

                if line.is_empty() {
                    continue;
                }
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                if data == "[DONE]" {
                    break;
                }

                let parsed: serde_json::Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if let Some(token) = parsed["choices"][0]["delta"]["content"].as_str() {
                    on_token(token)?;
                    full_response.push_str(token);
                }
            }
        }

        Ok(full_response)
    }
}
