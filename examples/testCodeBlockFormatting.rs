use anyhow::Result;
use loomiscli::cli::StreamingCodeFormatter;
use loomiscli::db::VectorStore;
use loomiscli::llm::{build_rag_messages, CodeIntent, LlmClient};
use loomiscli::sidecar::SidecarClient;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== LoomisCLI v1.1.6 Code Block Formatting Verification ===\n");

    println!("--- Part 1: Unit Test of StreamingCodeFormatter ---");
    let mut formatter = StreamingCodeFormatter::new();
    let simulated_stream = vec![
        "Here is the ",
        "requested code ",
        "solution:\n\n",
        "```python\n",
        "import hashlib\n",
        "import os\n\n",
        "# Compute md5 hash of string data\n",
        "def mkmd5sum(data: str) -> str:\n",
        "    checksum = hashlib.md5()\n",
        "    checksum.update(data.encode('utf-8'))\n",
        "    return checksum.hexdigest()\n",
        "```\n",
        "\nThis function calculates the MD5 digest directly.",
    ];

    for chunk in simulated_stream {
        formatter.process_chunk(chunk)?;
    }
    formatter.finish()?;
    println!("\n[PASS] Part 1: Formatter simulated stream completed successfully.\n");

    println!("--- Part 2: Live LLM RAG Streaming Test with Code Block Framing ---");
    let llm = LlmClient::new(
        "http://localhost:8080/v1/chat/completions".to_string(),
        "llama-3.2-1b".to_string(),
        None,
    );
    let mut sidecar = SidecarClient::new().await?;
    let store = VectorStore::connect_or_create().await?;

    let q = "give me code about m5 checksum";
    let intent = llm.classify_code_intent(q).await?;
    println!("Query: '{}' -> Intent: {:?}", q, intent);
    assert_eq!(intent, CodeIntent::Code);

    let emb = sidecar.embed(q).await?;
    let chunks = store.search(emb, 3).await?;
    let messages = build_rag_messages(q, &chunks, &[], intent);

    println!("\nStreaming live LLM response with formatted code blocks:");
    println!("-------------------------------------------------------");
    let mut live_formatter = StreamingCodeFormatter::new();
    let full_res = llm
        .stream_chat(&messages, |token| {
            live_formatter.process_chunk(token)?;
            Ok(())
        })
        .await?;
    live_formatter.finish()?;
    println!("-------------------------------------------------------");
    println!("Full unformatted text length captured in history: {} chars", full_res.len());
    assert!(!full_res.is_empty(), "Response must not be empty");

    println!("\n=== All v1.1.6 Code Formatting Checks Succeeded ===");
    Ok(())
}
