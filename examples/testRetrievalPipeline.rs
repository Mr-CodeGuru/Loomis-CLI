// examples/testRetrievalPipeline.rs
// Verifies Step 4 of claude.md:
// - Replaces the placeholder query vector with a REAL embedded query from the Python sidecar.
// - Performs vector search against LanceDB `chunks` table.
// - Displays top-5 matching code chunks with file path, symbol name, language, distance, and snippet.

use loomiscli::db::VectorStore;
use loomiscli::sidecar::SidecarClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Testing Real Query Vector Retrieval Pipeline ===\n");

    println!("1. Initializing LanceDB VectorStore...");
    let store = VectorStore::connect_or_create().await?;
    let total_chunks = store.count_rows().await?;
    println!("   LanceDB initialized. Total rows in 'chunks': {}", total_chunks);

    println!("\n2. Initializing Python Sidecar...");
    let mut sidecar = SidecarClient::new().await?;
    println!("   Python Sidecar ready.");

    let query = "calculate hash or SHA digest of data";
    println!("\n3. Embedding user query: \"{}\" ...", query);
    let query_vector = sidecar.embed(query).await?;
    println!("   Embedded query vector dimension: {}", query_vector.len());

    println!("\n4. Running vector search in LanceDB (top 5 matches)...");
    let results = store.search(query_vector, 5).await?;

    println!("\n=== Top Search Results ===");
    for (idx, item) in results.iter().enumerate() {
        println!(
            "\n[{}] Distance: {:.4} | Lang: {} | Symbol: {}",
            idx + 1,
            item.distance,
            item.language,
            if item.extracted_name.is_empty() { "<unnamed>" } else { &item.extracted_name }
        );
        println!("    Path: {}", item.path);
        let preview = item.text.lines().take(3).collect::<Vec<_>>().join("\n    ");
        println!("    Snippet:\n    {}", preview);
    }

    if results.is_empty() {
        anyhow::bail!("FAIL: Search returned zero results!");
    }

    println!("\nPASS: Real retrieval pipeline working end-to-end with Python sidecar embeddings!");
    sidecar.shutdown().await;

    Ok(())
}
