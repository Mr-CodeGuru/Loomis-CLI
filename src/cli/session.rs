use anyhow::Result;
use std::io::{self, Write};
use crate::config::AppConfig;
use crate::core::LoomisCore;
use crate::llm::CodeIntent;
use super::commands::CommandHandler;
use super::formatter::StreamingCodeFormatter;

pub struct ReplSession {
    pub core: LoomisCore,
}

impl ReplSession {
    pub async fn init(config: AppConfig) -> Result<Self> {
        println!("\nInitializing LoomisCLI...");

        print!("Booting Python embedding sidecar... ");
        io::stdout().flush()?;
        let core = match LoomisCore::init(config).await {
            Ok(c) => {
                println!("ready.");
                c
            }
            Err(e) => {
                println!("failed!");
                eprintln!("[FATAL] Could not initialize LoomisCore: {e}");
                return Err(e);
            }
        };

        // Pre-flight check to verify LLM server connectivity
        print!("Testing LLM endpoint ({}) ... ", core.config.endpoint_url);
        io::stdout().flush()?;
        match core.llm.test_connection().await {
            Ok(_) => println!("connected."),
            Err(e) => {
                println!("warning: could not reach LLM server ({e}).");
                println!("         Make sure llama-server is running on {}", core.config.endpoint_url);
            }
        }

        Ok(Self { core })
    }

    pub async fn run(&mut self) -> Result<()> {
        self.run_loop().await
    }

    async fn run_loop(&mut self) -> Result<()> {
        Self::print_banner(&self.core.config);

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
                    &self.core.config,
                    &self.core.vector_store,
                    &mut self.core.history,
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
        self.core.sidecar.shutdown().await;
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
        // 1. Classify intent and conditionally retrieve chunks
        let prep = self.core.prepare_query(query).await?;

        match prep.intent {
            CodeIntent::Chat => {
                println!("[Intent: CHAT -> Non-code query detected (RAG bypassed)]");
            }
            CodeIntent::Code => {
                println!("[Intent: CODE -> Code query detected (RAG retrieval initiated)]");
                println!("🔍 Searching repository context... done. (found {} relevant snippets)", prep.chunks.len());

                if !prep.chunks.is_empty() {
                    println!("\nRetrieved Context Sources:");
                    for (idx, item) in prep.chunks.iter().enumerate() {
                        let name = if item.extracted_name.is_empty() { "block" } else { &item.extracted_name };
                        println!("  [{}] {} ({}) [dist: {:.2}]", idx + 1, item.path, name, item.distance);
                    }
                }
            }
        }

        // 2. Send to LLM for regeneration
        println!("\n--- Loomis ---");
        let mut formatter = StreamingCodeFormatter::new();
        let stream_result = self
            .core
            .llm
            .stream_chat(&prep.messages, |token| {
                formatter.process_chunk(token)?;
                Ok(())
            })
            .await;

        formatter.finish()?;
        println!(); // trailing newline

        match stream_result {
            Ok(full_response) => {
                self.core.record_turn(query, &full_response);
            }
            Err(e) => {
                eprintln!("\n[ERROR] LLM generation failed: {e}");
                eprintln!("        Make sure llama-server is running on {}", self.core.config.endpoint_url);
            }
        }

        Ok(())
    }
}
