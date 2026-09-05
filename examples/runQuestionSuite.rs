use loomiscli::db::VectorStore;
use loomiscli::llm::{build_rag_messages, fallback_classify_code_intent, CodeIntent, LlmClient};
use loomiscli::sidecar::SidecarClient;

struct TestCase {
    id: &'static str,
    category: &'static str,
    query: &'static str,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("============================================================");
    println!("   LoomisCLI Test Question Suite (v1.0.3 Allow-list Intent)");
    println!("============================================================\n");

    let store = VectorStore::connect_or_create().await?;
    let mut sidecar = SidecarClient::new().await?;
    let llm = LlmClient::new(
        "http://localhost:8080/v1/chat/completions".to_string(),
        "llama-3.2-1b".to_string(),
        None,
    );

    let test_cases = vec![
        // Varied phrasing greetings & casual chat (Must bypass retrieval & generate NO code)
        TestCase {
            id: "G1",
            category: "Greeting / Literal",
            query: "hello",
        },
        TestCase {
            id: "G2-REG",
            category: "Greeting / Natural Phrasing (v1.0.2 Regression Case)",
            query: "first time here, so hello",
        },
        TestCase {
            id: "G3",
            category: "Greeting / Capabilities Inquiry",
            query: "who are you and what can you do?",
        },
        TestCase {
            id: "G4",
            category: "Greeting / Conversational",
            query: "good evening, how are you today?",
        },
        TestCase {
            id: "G5",
            category: "General Non-Code Question",
            query: "what is the capital of France",
        },

        // Question-form code requests (Must classify as CODE & trigger retrieval)
        TestCase {
            id: "Q1",
            category: "Question-form Code Request",
            query: "how would you write a function that calculates the sha256 hash of a file",
        },
        TestCase {
            id: "Q2",
            category: "Question-form Code Request",
            query: "can you implement a context manager that logs execution time",
        },

        // Part B: Code generation suite (Must classify as CODE & trigger retrieval)
        TestCase {
            id: "B1",
            category: "Part B (Imperative Code Generation)",
            query: "Write a Python function that recursively walks a directory tree and returns all .py files, following the conventions used elsewhere in this codebase for path handling.",
        },
        TestCase {
            id: "B2",
            category: "Part B (Imperative Code Extension)",
            query: "Extend the stop() function pattern from turtledemo into a full pause/resume state machine for an animation loop.",
        },
        TestCase {
            id: "B3",
            category: "Part B (Imperative Code Generation)",
            query: "Implement a custom context manager that logs entry/exit timing, following whatever context-manager patterns already exist in this codebase.",
        },
        TestCase {
            id: "B4",
            category: "Part B (Imperative Code Generation)",
            query: "Given the error-handling style used in this codebase, write a function that safely parses a config file and raises a custom exception with a helpful message on failure.",
        },
        TestCase {
            id: "B5",
            category: "Part B (Imperative Code Refactoring)",
            query: "Refactor a hypothetical function that uses nested loops for a Cartesian product into something more idiomatic, based on patterns you can find in this codebase (e.g. itertools usage).",
        },
    ];

    for tc in test_cases {
        println!("\n------------------------------------------------------------");
        println!("Test ID: {} | {}", tc.id, tc.category);
        println!("Query: \"{}\"", tc.query);

        // Classify intent via fast LLM pass with fallback
        let intent = match llm.classify_code_intent(tc.query).await {
            Ok(i) => i,
            Err(_) => fallback_classify_code_intent(tc.query),
        };

        match intent {
            CodeIntent::Chat => {
                println!("[Intent Decision: CHAT -> Direct response (Search bypassed)]");
            }
            CodeIntent::Code => {
                println!("[Intent Decision: CODE -> Code request detected (Running repository search)]");
            }
        }

        let chunks = if intent == CodeIntent::Chat {
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

        let messages = build_rag_messages(tc.query, &chunks, &[], intent);

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
    println!("============================================================\n");
    Ok(())
}
