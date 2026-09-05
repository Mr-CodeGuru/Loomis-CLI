use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use super::prompt::CodeIntent;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone)]
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

    /// Query intent classifier using local model few-shot evaluation.
    /// Returns CodeIntent::Code if user is asking for code generation or implementation,
    /// or CodeIntent::Chat for greetings, general non-code conversation, or conceptual questions.
    pub async fn classify_code_intent(&self, query: &str) -> Result<CodeIntent> {
        let classification_messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a strict intent classification engine. Classify the user query into exactly one word: CODE or CHAT.\n- CODE: user requests code implementation, generation, refactoring, or writing code.\n- CHAT: conversational greetings, general non-code questions, conceptual explanations without code request, or asking about chat history.\n\nAnswer with ONLY 'CODE' or 'CHAT'.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "User query: hello\nClassification (CODE or CHAT):".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "CHAT".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "User query: give me code about m5 checksum\nClassification (CODE or CHAT):".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "CODE".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "User query: explain how a checksum works\nClassification (CODE or CHAT):".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "CHAT".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "User query: what is the difference between list and tuple\nClassification (CODE or CHAT):".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "CHAT".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "User query: what's my last prompt to you?\nClassification (CODE or CHAT):".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "CHAT".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "User query: now make it faster\nClassification (CODE or CHAT):".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "CODE".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!("User query: {query}\nClassification (CODE or CHAT):"),
            },
        ];

        let req_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: classification_messages,
            stream: false,
            temperature: 0.0,
            max_tokens: 4,
        };

        let mut req = self.client.post(&self.endpoint_url).json(&req_body);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req.send().await.context("Failed to send classification request")?;
        if !resp.status().is_success() {
            return Ok(CodeIntent::Chat);
        }

        let body: ChatCompletionResponse = resp.json().await.context("Failed to parse classification response")?;
        let answer = body
            .choices
            .first()
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.trim().to_uppercase())
            .unwrap_or_default();

        if answer.contains("CODE") {
            Ok(CodeIntent::Code)
        } else {
            Ok(CodeIntent::Chat)
        }
    }

    /// Pre-flight connectivity check to verify LLM server is up and reachable.
    pub async fn test_connection(&self) -> Result<()> {
        let test_messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "ping".to_string(),
        }];

        let req_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: test_messages,
            stream: false,
            temperature: 0.1,
            max_tokens: 5,
        };

        let mut req = self.client.post(&self.endpoint_url).json(&req_body);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req
            .send()
            .await
            .context("Cannot reach LLM server. Is llama-server running?")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM server returned HTTP {status}: {text}");
        }

        Ok(())
    }

    /// Stream chat response from OpenAI-compatible endpoint using Server-Sent Events (SSE).
    /// Calls `on_token` callback synchronously for each streamed token piece.
    /// Returns the complete accumulated response text.
    pub async fn stream_chat<F>(&self, messages: &[ChatMessage], mut on_token: F) -> Result<String>
    where
        F: FnMut(&str) -> Result<(), std::io::Error>,
    {
        let req_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            stream: true,
            temperature: 0.2,
            max_tokens: 1536,
        };

        let mut req = self.client.post(&self.endpoint_url).json(&req_body);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req
            .send()
            .await
            .context("Failed to connect to LLM server for streaming")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM streaming request failed with HTTP {status}: {text}");
        }

        let mut stream = resp.bytes_stream();
        let mut full_response = String::new();
        let mut line_buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.context("Error reading SSE stream chunk")?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            line_buffer.push_str(&chunk_str);

            while let Some(pos) = line_buffer.find('\n') {
                let line: String = line_buffer.drain(..=pos).collect();
                let trimmed = line.trim();

                if trimmed.is_empty() || trimmed.starts_with(':') {
                    continue; // Skip keep-alive / SSE comments
                }

                if let Some(data_str) = trimmed.strip_prefix("data:") {
                    let data = data_str.trim();

                    if data == "[DONE]" {
                        break;
                    }

                    if let Ok(stream_chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                        for choice in stream_chunk.choices {
                            if let Some(content) = choice.delta.content {
                                if !content.is_empty() {
                                    on_token(&content)?;
                                    full_response.push_str(&content);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(full_response)
    }
}

// Request and response DTOs matching OpenAI Chat Completion schema
#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChoiceResponse>,
}

#[derive(Deserialize)]
struct ChoiceResponse {
    message: Option<ChoiceMessage>,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: DeltaContent,
}

#[derive(Deserialize)]
struct DeltaContent {
    content: Option<String>,
}
