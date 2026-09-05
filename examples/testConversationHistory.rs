use anyhow::Result;
use loomiscli::db::VectorStore;
use loomiscli::llm::{build_rag_messages, ChatMessage, CodeIntent, LlmClient};
use loomiscli::sidecar::SidecarClient;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== LoomisCLI v1.1.4 Verification Suite ===\n");

    let llm = LlmClient::new(
        "http://localhost:8080/v1/chat/completions".to_string(),
        "llama-3.2-1b".to_string(),
        None,
    );

    println!("--- Part 1: Intent Classification Phrasing Matrix ---");
    let test_queries = [
        ("give me code about m5 checksum", CodeIntent::Code, "Regression from v1.1.4"),
        ("give me code for md5 checksum", CodeIntent::Code, "Direct code request variant"),
        ("show me code to read a file", CodeIntent::Code, "Direct code request variant"),
        ("need code for quicksort", CodeIntent::Code, "Code request variant"),
        ("how would you write a function to parse json", CodeIntent::Code, "Question-form code request"),
        ("can you write a script that deletes old logs", CodeIntent::Code, "Question-form code request"),
        ("refactor this loop into a list comprehension", CodeIntent::Code, "Refactoring request"),
        ("Write a Python function that recursively walks a directory tree and returns all .py files", CodeIntent::Code, "Part B baseline"),
        ("Extend the stop() function pattern from turtledemo into a full pause/resume state machine", CodeIntent::Code, "Part B baseline"),
        ("now make it faster", CodeIntent::Code, "Multi-turn follow-up code request"),
        ("hello", CodeIntent::Chat, "Greeting baseline"),
        ("first time here, so hello", CodeIntent::Chat, "v1.0.3 regression baseline"),
        ("what can you do", CodeIntent::Chat, "Capabilities inquiry"),
        ("sooo, how are you?", CodeIntent::Chat, "Small-talk inquiry"),
        ("what's my last prompt to you?", CodeIntent::Chat, "Meta-conversation inquiry"),
        ("what is the capital of France", CodeIntent::Chat, "Non-code general knowledge"),
    ];

    let mut passed = 0;
    for (q, expected, label) in &test_queries {
        let actual = llm.classify_code_intent(q).await?;
        let ok = actual == *expected;
        if ok {
            passed += 1;
        }
        let status = if ok { "PASS" } else { "FAIL" };
        println!(
            "[{status}] {:?} (expected {:?}) <- '{}' [{label}]",
            actual, expected, q
        );
    }
    println!("\nClassification Accuracy: {}/{}\n", passed, test_queries.len());
    assert_eq!(passed, test_queries.len(), "All classification tests must pass!");

    println!("--- Part 2: Multi-Turn Conversation History Retention ---");
    let mut history: Vec<ChatMessage> = Vec::new();

    // Turn 1: Greeting
    let q1 = "sooo, how are you?";
    let intent1 = llm.classify_code_intent(q1).await?;
    println!("Turn 1: User says: '{}' [Intent: {:?}]", q1, intent1);
    let msgs1 = build_rag_messages(q1, &[], &history, intent1);
    let r1 = llm.stream_chat(&msgs1, |_| Ok(())).await?;
    println!("Loomis Turn 1 response: {}\n", r1.trim());
    history.push(ChatMessage { role: "user".to_string(), content: q1.to_string() });
    history.push(ChatMessage { role: "assistant".to_string(), content: r1 });

    // Turn 2: Meta-prompt check: "what's my last prompt to you?"
    let q2 = "what's my last prompt to you?";
    let intent2 = llm.classify_code_intent(q2).await?;
    println!("Turn 2: User says: '{}' [Intent: {:?}]", q2, intent2);
    let msgs2 = build_rag_messages(q2, &[], &history, intent2);
    let r2 = llm.stream_chat(&msgs2, |_| Ok(())).await?;
    println!("Loomis Turn 2 response: {}\n", r2.trim());
    let low_r2 = r2.to_lowercase();
    let references_last = low_r2.contains("how are you") || low_r2.contains("doing") || low_r2.contains("sooo");
    println!("Verified Turn 2 references previous prompt correctly: {}", references_last);
    assert!(references_last, "Turn 2 should correctly identify the prior prompt from session history!");
    history.push(ChatMessage { role: "user".to_string(), content: q2.to_string() });
    history.push(ChatMessage { role: "assistant".to_string(), content: r2 });

    // Turn 3: Unambiguous Code Request: "give me code about m5 checksum"
    let q3 = "give me code about m5 checksum";
    let intent3 = llm.classify_code_intent(q3).await?;
    println!("Turn 3: User says: '{}' [Intent: {:?}]", q3, intent3);
    assert_eq!(intent3, CodeIntent::Code, "Turn 3 must be classified as CODE!");

    // Connect sidecar and store for RAG
    let mut sidecar = SidecarClient::new().await?;
    let store = VectorStore::connect_or_create().await?;

    let emb = sidecar.embed(q3).await?;
    let chunks = store.search(emb, 5).await?;
    println!("Retrieved {} chunks for '{}'", chunks.len(), q3);
    for (i, c) in chunks.iter().enumerate() {
        println!("  [{}] {} (dist: {:.2})", i + 1, c.path, c.distance);
    }

    let msgs3 = build_rag_messages(q3, &chunks, &history, intent3);
    let r3 = llm.stream_chat(&msgs3, |_| Ok(())).await?;
    println!("Loomis Turn 3 response:\n{}\n", r3.trim());

    // Turn 4: Follow-up code request: "now make it faster"
    let q4 = "now make it faster";
    let intent4 = llm.classify_code_intent(q4).await?;
    println!("Turn 4: User says: '{}' [Intent: {:?}]", q4, intent4);
    assert_eq!(intent4, CodeIntent::Code, "Turn 4 must be classified as CODE!");

    let emb4 = sidecar.embed(q4).await?;
    let chunks4 = store.search(emb4, 5).await?;
    let msgs4 = build_rag_messages(q4, &chunks4, &history, intent4);
    let r4 = llm.stream_chat(&msgs4, |_| Ok(())).await?;
    println!("Loomis Turn 4 response:\n{}\n", r4.trim());

    println!("=== All v1.1.4 Checks Succeeded ===");
    Ok(())
}
