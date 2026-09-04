use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub endpoint_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

fn default_top_k() -> usize {
    5
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            endpoint_url: "http://localhost:8080/v1/chat/completions".to_string(),
            model: "llama-3.2-1b".to_string(),
            api_key: None,
            top_k: 5,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> Result<PathBuf> {
        let home = if cfg!(windows) {
            std::env::var("USERPROFILE").map(PathBuf::from)
        } else {
            std::env::var("HOME").map(PathBuf::from)
        }
        .context("Failed to determine user home directory")?;

        Ok(home.join(".loomiscli").join("config.json"))
    }

    pub fn load_or_prompt() -> Result<Self> {
        let path = Self::config_path()?;
        if path.is_file() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file at: {}", path.display()))?;
            let config: AppConfig = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse config file at: {}", path.display()))?;
            return Ok(config);
        }

        println!("=== LoomisCLI First-Run Configuration ===");
        println!("No config file found at {}", path.display());

        print!("Enter LLM endpoint base URL [default: http://localhost:8080]: ");
        io::stdout().flush()?;
        let mut base_url = String::new();
        io::stdin().read_line(&mut base_url)?;
        let base_url = base_url.trim();

        let endpoint_url = if base_url.is_empty() {
            "http://localhost:8080/v1/chat/completions".to_string()
        } else if base_url.ends_with("/v1/chat/completions") {
            base_url.to_string()
        } else {
            format!("{}/v1/chat/completions", base_url.trim_end_matches('/'))
        };

        print!("Enter optional API key (press Enter to skip): ");
        io::stdout().flush()?;
        let mut api_key_input = String::new();
        io::stdin().read_line(&mut api_key_input)?;
        let api_key_trimmed = api_key_input.trim();
        let api_key = if api_key_trimmed.is_empty() {
            None
        } else {
            Some(api_key_trimmed.to_string())
        };

        let config = AppConfig {
            endpoint_url,
            model: "llama-3.2-1b".to_string(),
            api_key,
            top_k: 5,
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_string_pretty(&config)?;
        fs::write(&path, serialized)?;
        println!("Configuration saved to: {}\n", path.display());

        Ok(config)
    }
}
