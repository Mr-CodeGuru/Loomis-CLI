use anyhow::Result;
use std::io::{self, Write};
use crate::config::AppConfig;
use crate::db::VectorStore;
use crate::llm::{build_rag_messages, fallback_classify_code_intent, ChatMessage, CodeIntent, LlmClient};
use crate::sidecar::SidecarClient;
use super::commands::CommandHandler;

pub struct ReplSession {
    config: AppConfig,
    vector_store: VectorStore,
    sidecar: SidecarClient,
    llm: LlmClient,
    history: Vec<ChatMessage>,
}

impl ReplSession {
    pub async fn init(config: AppConfig) -> Result<Self> {
        println!("\nInitializing LoomisCLI...");

        // 1. LanceDB vector store
        let vector_store = VectorStore::connect_or_create().await?;

        // 2. Python sidecar IPC
        print!("Booting Python embedding sidecar... ");
        io::stdout().flush()?;
        let sidecar = match SidecarClient::new().await {
            Ok(c) => {
                println!("ready.");
                c
            }
            Err(e) => {
                println!("failed!");
                eprintln!("[FATAL] Could not launch Python sidecar: {e}");
                eprintln!("        Run .\\setup.ps1 (or bash setup.sh) to verify Python venv & model download.");
                return Err(e);
            }
        };

        // 3. LLM client
        let llm = LlmClient::new(
            config.endpoint_url.clone(),
            config.model.clone(),
            config.api_key.clone(),
        );

        // Pre-flight check to verify LLM server connectivity
        print!("Testing LLM endpoint ({}) ... ", config.endpoint_url);
        io::stdout().flush()?;
        match llm.test_connection().await {
            Ok(_) => println!("connected."),
            Err(e) => {
                println!("warning: could not reach LLM server ({e}).");
                println!("         Make sure llama-server is running on {}", config.endpoint_url);
            }
        }

        Ok(Self {
            config,
            vector_store,
            sidecar,
            llm,
            history: Vec::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        self.run_loop().await
    }

    async fn run_loop(&mut self) -> Result<()> {
        Self::print_banner(&self.config);

        loop {
            print!("\nloomis> ");
            io::stdout().flush()?;

            let mut input = String::new();
            if io::stdin().read_line(&mut input)? == 0 {
                // EOF reached (Ctrl+D / Ctrl+Z)
                break;
            }

            let trimmed = input.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check slash commands
            if trimmed.starts_with('/') {
                let should_continue = CommandHandler::execute(
                    trimmed,
                    &self.config,
                    &self.vector_store,
                    &mut self.history,
                )
                .await?;

                if !should_continue {
                    break;
                }
                continue;
            }

            // Standard RAG query pipeline
            self.handle_query(trimmed).await?;
        }

        println!("\nShutting down sidecar...");
        self.sidecar.shutdown().await;
        println!("Goodbye!");
        Ok(())
    }

    pub fn print_banner(config: &AppConfig) {
        println!("\n===========================================================");
        println!("  LoomisCLI — Local RAG Code Assistant");
        println!("  Endpoint: {}", config.endpoint_url);
        println!("  Model:    {} (top-k chunks: {})", config.model, config.top_k);
        println!("  Type your code question or slash commands (/help, /exit)");
        println!("===========================================================");
    }

    async fn handle_query(&mut self, query: &str) -> Result<()> {
        // 1. Binary intent classification via fast LLM pass
        let intent = match self.llm.classify_code_intent(query).await {
            Ok(i) => i,
            Err(_) => fallback_classify_code_intent(query),
        };

        // 2. Visible logging of classification decision
        match intent {
            CodeIntent::Chat => {
                println!("[Intent: CHAT -> Direct response (Search bypassed)]");
            }
            CodeIntent::Code => {
                println!("[Intent: CODE -> Code request detected (Running repository search)]");
            }
        }

        // 3. Conditional retrieval: only CODE intent triggers LanceDB search
        let chunks = if intent == CodeIntent::Chat {
            Vec::new()
        } else {
            print!("🔍 Searching repository context... ");
            io::stdout().flush()?;

            // Compute embedding with sidecar
            let query_vector = match self.sidecar.embed(query).await {
                Ok(v) => v,
                Err(e) => {
                    println!("\n[ERROR] Embedding failed: {e}");
                    return Ok(());
                }
            };

            // Query LanceDB for top-K chunks
            let c = match self.vector_store.search(query_vector, self.config.top_k).await {
                Ok(c) => c,
                Err(e) => {
                    println!("\n[ERROR] Vector search failed: {e}");
                    return Ok(());
                }
            };

            println!("done. (found {} relevant snippets)", c.len());

            if !c.is_empty() {
                println!("\nRetrieved Context Sources:");
                for (idx, item) in c.iter().enumerate() {
                    let name = if item.extracted_name.is_empty() { "block" } else { &item.extracted_name };
                    println!("  [{}] {} ({}) [dist: {:.2}]", idx + 1, item.path, name, item.distance);
                }
            }
            c
        };

        // 4. Build RAG prompt
        let messages = build_rag_messages(query, &chunks, &self.history, intent);

        // 5. Stream response from LLM
        println!("\n--- Loomis ---");
        let stream_result = self
            .llm
            .stream_chat(&messages, |token| {
                print!("{token}");
                io::stdout().flush()?;
                Ok(())
            })
            .await;

        println!(); // trailing newline

        match stream_result {
            Ok(full_response) => {
                // Update ephemeral conversation history (never saved to disk)
                // Only push constructive answers; skip refusals from poisoning history
                if !full_response.trim().starts_with("I can't answer that") {
                    self.history.push(ChatMessage {
                        role: "user".to_string(),
                        content: query.to_string(),
                    });
                    self.history.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: full_response,
                    });
                }
            }
            Err(e) => {
                eprintln!("\n[ERROR] LLM generation failed: {e}");
                eprintln!("        Make sure llama-server is running on {}", self.config.endpoint_url);
            }
        }

        Ok(())
    }
}
