use loomiscli::db::VectorStore;
use loomiscli::llm::{build_rag_messages, classify_query_intent, LlmClient, QueryIntent};
use loomiscli::sidecar::SidecarClient;

struct TestCase {
    id: &'static str,
    part: &'static str,
    query: &'static str,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("============================================================");
    println!("   LoomisCLI Test Question Suite (TQ1v1.0.2) Runner");
    println!("============================================================\n");

    let store = VectorStore::connect_or_create().await?;
    let mut sidecar = SidecarClient::new().await?;
    let llm = LlmClient::new(
        "http://localhost:8080/v1/chat/completions".to_string(),
        "llama-3.2-1b".to_string(),
        None,
    );

    let test_cases = vec![
        // Greetings
        TestCase {
            id: "G1",
            part: "Greeting",
            query: "hello",
        },
        TestCase {
            id: "G2",
            part: "Greeting",
            query: "hi, who are you and what can you do?",
        },
        // Part A
        TestCase {
            id: "A1",
            part: "Part A (Retrieval)",
            query: "function that stops a running loop",
        },
        TestCase {
            id: "A2",
            part: "Part A (Retrieval)",
            query: "def stop",
        },
        TestCase {
            id: "A3",
            part: "Part A (Retrieval)",
            query: "how to handle a KeyError when accessing a dictionary",
        },
        TestCase {
            id: "A4",
            part: "Part A (Retrieval)",
            query: "turtle graphics animation",
        },
        TestCase {
            id: "A5",
            part: "Part A (Retrieval)",
            query: "recursive function with memoization",
        },
        // Part B
        TestCase {
            id: "B1",
            part: "Part B (Generation)",
            query: "Write a Python function that recursively walks a directory tree and returns all .py files, following the conventions used elsewhere in this codebase for path handling.",
        },
        TestCase {
            id: "B2",
            part: "Part B (Generation)",
            query: "Extend the stop() function pattern from turtledemo into a full pause/resume state machine for an animation loop.",
        },
        TestCase {
            id: "B3",
            part: "Part B (Generation)",
            query: "Implement a custom context manager that logs entry/exit timing, following whatever context-manager patterns already exist in this codebase.",
        },
        TestCase {
            id: "B4",
            part: "Part B (Generation)",
            query: "Given the error-handling style used in this codebase, write a function that safely parses a config file and raises a custom exception with a helpful message on failure.",
        },
        TestCase {
            id: "B5",
            part: "Part B (Generation)",
            query: "Refactor a hypothetical function that uses nested loops for a Cartesian product into something more idiomatic, based on patterns you can find in this codebase (e.g. itertools usage).",
        },
    ];

    for tc in test_cases {
        println!("\n------------------------------------------------------------");
        println!("Test ID: {} | {}", tc.id, tc.part);
        println!("Query: \"{}\"", tc.query);

        let intent = classify_query_intent(tc.query);
        println!("Intent Detected: {:?}", intent);

        let chunks = if intent == QueryIntent::Greeting {
            println!("(Greeting detected -> skipping LanceDB vector search)");
            Vec::new()
        } else {
            let vec = sidecar.embed(tc.query).await?;
            let res = store.search(vec, 5).await?;
            println!("Retrieved {} sources:", res.len());
            for (idx, c) in res.iter().enumerate() {
                println!(
                    "  [{}] dist: {:.2} | file: {} | symbol: {}",
                    idx + 1,
                    c.distance,
                    c.path,
                    if c.extracted_name.is_empty() { "<unnamed>" } else { &c.extracted_name }
                );
            }
            res
        };

        let messages = build_rag_messages(tc.query, &chunks, &[]);

        print!("\nLLM Output: ");
        let response = llm.stream_chat(&messages, |t| {
            print!("{t}");
            use std::io::Write;
            let _ = std::io::stdout().flush();
            Ok(())
        }).await?;
        println!("\n[End of response. Length: {} chars]", response.len());
    }

    sidecar.shutdown().await;
    println!("\n============================================================");
    println!("   All test cases finished!");
    println!("============================================================");
    Ok(())
}
