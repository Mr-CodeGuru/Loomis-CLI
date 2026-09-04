// examples/testLlamaStreaming.rs
// Tests whether llama-server's /v1/chat/completions supports streaming (stream: true),
// parsing the Server-Sent-Events (SSE) response as it arrives, token by token.
//
// Prerequisite: llama-server must already be running, e.g.:
//   llama-server -m models\Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080
//
// Usage: cargo run --example testLlamaStreaming
//
// NOT YET RUN — SSE parsing here is a straightforward line-by-line split on "data: " prefixes,
// which matches the standard OpenAI-compatible streaming format, but hasn't been verified against
// llama-server's actual streaming output. If chunks arrive split mid-line (a real possibility with
// raw byte streams), the naive line-splitting here may need a proper buffer — flag if that happens
// rather than assuming this handles it correctly.

use futures_util::StreamExt;
use serde_json::json;
use std::io::Write;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let endpoint = "http://localhost:8080/v1/chat/completions";

    let payload = json!({
        "model": "llama-3.2-1b",
        "messages": [
            {"role": "user", "content": "Count from 1 to 5, one number per line."}
        ],
        "stream": true
    });

    println!("Sending streaming request to {endpoint} ...\n");

    let client = reqwest::Client::new();
    let response = client.post(endpoint).json(&payload).send().await?;

    let status = response.status();
    println!("Status: {status}");

    if !status.is_success() {
        let body = response.text().await?;
        println!("FAIL: non-success status. Body: {body}");
        return Ok(());
    }

    println!("=== Streamed tokens ===");

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut received_any_token = false;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // Process complete lines; keep any trailing partial line in the buffer for next chunk.
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
                continue;
            }

            let parsed: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(e) => {
                    println!("\n[WARN: failed to parse SSE chunk as JSON: {e}; raw: {data}]");
                    continue;
                }
            };

            if let Some(token) = parsed["choices"][0]["delta"]["content"].as_str() {
                print!("{token}");
                std::io::stdout().flush()?;
                received_any_token = true;
            }
        }
    }

    println!("\n");
    if received_any_token {
        println!("PASS: received streamed tokens.");
    } else {
        println!("FAIL: no token content received — check response format above.");
    }

    Ok(())
}