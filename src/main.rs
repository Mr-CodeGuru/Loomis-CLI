use loomiscli::cli::ReplSession;
use loomiscli::config::AppConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load_or_prompt()?;
    let mut session = ReplSession::init(config).await?;
    session.run().await?;
    Ok(())
}
