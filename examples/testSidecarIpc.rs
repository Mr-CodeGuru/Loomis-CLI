// examples/testSidecarIpc.rs
// Verifies Step 2 of claude.md:
// - Rust spawns the Python sidecar process.
// - Confirms round-trip embedding request (768 dimensions).
// - Deliberately kills the sidecar process mid-run.
// - Confirms Rust detects the dead process, restarts it, and successfully fulfills the next request.

use loomiscli::sidecar::SidecarClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Testing Python Sidecar IPC & Supervision ===");

    println!("1. Spawning sidecar client and waiting for ready signal...");
    let mut client = SidecarClient::new().await?;
    println!("   Sidecar initialized successfully.");

    println!("\n2. Sending ping request...");
    client.ping().await?;
    println!("   Ping received pong.");

    println!("\n3. Sending sample embedding request...");
    let sample_text = "def add(a: int, b: int) -> int:\n    return a + b";
    let emb = client.embed(sample_text).await?;
    println!("   Received embedding vector with dimension: {}", emb.len());
    println!("   First 5 values: {:?}", &emb[..5]);

    if emb.len() != 768 {
        anyhow::bail!("FAIL: Expected 768 dimensions, got {}", emb.len());
    }
    println!("   PASS: Round-trip embedding verified (768 dimensions).");

    println!("\n4. Testing intentional crash & auto-restart behavior...");
    println!("   Forcibly killing sidecar child process...");
    client.kill_raw_child_for_testing().await?;
    println!("   Child process killed.");

    println!("   Sending new embedding request (expecting auto-restart)...");
    let sample_text_2 = "fn compute_hash(data: &[u8]) -> u64";
    let emb_2 = client.embed(sample_text_2).await?;
    println!("   Received embedding vector with dimension: {}", emb_2.len());
    println!("   First 5 values: {:?}", &emb_2[..5]);

    if emb_2.len() != 768 {
        anyhow::bail!("FAIL: Expected 768 dimensions on restart, got {}", emb_2.len());
    }

    println!("\nPASS: Sidecar auto-recovery and mid-run restart verified successfully!");
    client.shutdown().await;

    Ok(())
}
