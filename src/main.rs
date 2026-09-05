use loomiscli::cli::ReplSession;
use loomiscli::config::AppConfig;
use loomiscli::core::LoomisCore;
use loomiscli::tui::TuiApp;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let is_tui = args.iter().any(|a| a == "--tui");

    let config = AppConfig::load_or_prompt()?;

    if is_tui {
        let core = LoomisCore::init(config).await?;
        let mut app = TuiApp::new(core);
        app.run().await?;
    } else {
        let mut session = ReplSession::init(config).await?;
        session.run().await?;
    }

    Ok(())
}
