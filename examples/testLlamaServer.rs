// examples/testLlamaServer.rs
// Confirms llama-server actually returns a real completion via its OpenAI-compatible
// /v1/chat/completions endpoint — a real request/response test, not just "did it start."
// Doubles as an early proof-of-concept for the eventual Rust HTTP client.
//
// Prerequisite: llama-server must already be running, e.g.:
//   llama-server -m models\Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080
//
// Usage: cargo run --example testLlamaServer

use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let endpoint = "http://localhost:8080/v1/chat/completions";

    let payload = json!({
        "model": "llama-3.2-1b",
        "messages": [
            {"role": "user", "content": "Say OK if you can read this."}
        ]
    });

    println!("Sending request to {endpoint} ...");

    let client = reqwest::Client::new();
    let response = client
        .post(endpoint)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    println!("Status: {status}");

    let body: serde_json::Value = response.json().await?;
    println!("\n=== Full response ===");
    println!("{}", serde_json::to_string_pretty(&body)?);

    // Try to pull out just the completion text for a quick pass/fail signal.
    if let Some(content) = body["choices"][0]["message"]["content"].as_str() {
        println!("\n=== Completion text ===");
        println!("{content}");
        println!("\nPASS: received a real completion.");
    } else {
        println!("\nFAIL: response didn't contain the expected choices[0].message.content field.");
    }

    Ok(())
}