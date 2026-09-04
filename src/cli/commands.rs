use crate::config::AppConfig;
use crate::db::VectorStore;
use crate::llm::ChatMessage;
use anyhow::Result;
use std::io::{self, Write};

pub struct CommandHandler;

impl CommandHandler {
    pub async fn execute(
        cmd: &str,
        config: &AppConfig,
        vector_store: &VectorStore,
        history: &mut Vec<ChatMessage>,
    ) -> Result<bool> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        match parts[0] {
            "/exit" | "/quit" => {
                return Ok(true);
            }
            "/help" => {
                println!("\nAvailable commands:");
                println!("  /help     - Show this help message");
                println!("  /clear    - Clear conversation session history and screen");
                println!("  /stats    - Display store and sidecar statistics");
                println!("  /config   - Show active endpoint configuration");
                println!("  /exit     - Exit LoomisCLI (aliases: /quit)");
            }
            "/clear" => {
                history.clear();
                print!("\x1B[2J\x1B[1;1H");
                io::stdout().flush()?;
                println!("Session history cleared.");
            }
            "/stats" => {
                let rows = vector_store.count_rows().await.unwrap_or(0);
                println!("\nSystem Status:");
                println!("  LanceDB 'chunks' row count: {}", rows);
                println!("  Sidecar process: ACTIVE");
                println!("  Current session turns: {}", history.len() / 2);
            }
            "/config" => {
                println!("\nActive Configuration:");
                println!("  Endpoint URL: {}", config.endpoint_url);
                println!("  Model:        {}", config.model);
                println!("  Top-K:        {}", config.top_k);
                println!("  API Key:      {}", if config.api_key.is_some() { "[SET]" } else { "[NONE]" });
                if let Ok(path) = AppConfig::config_path() {
                    println!("  Config path:  {}", path.display());
                }
            }
            _ => {
                println!("Unknown command: '{}'. Type /help for available commands.", parts[0]);
            }
        }
        Ok(false)
    }
}
