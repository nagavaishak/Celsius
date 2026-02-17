mod config;
mod data;
mod strategies;
mod execution;
mod ai;
mod monitoring;

use anyhow::Result;
use config::{Config, EnvConfig};
use execution::persistence::PositionDatabase;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    tracing::info!("🚀 Polymarket Bot starting...");
    tracing::info!("📊 Phase 0: Infrastructure setup");

    // Load configuration
    tracing::info!("Loading configuration...");
    let config = Config::load("config.toml")?;
    let env_config = EnvConfig::load()?;

    tracing::info!("Dry run mode: {}", config.system.dry_run);
    tracing::info!("Paper trading: {}", config.paper_trading.enabled);
    tracing::info!("Weather strategy: {}", config.strategies.weather.enabled);
    tracing::info!("Arbitrage strategy: {}", config.strategies.arbitrage.enabled);

    // Initialize database
    tracing::info!("Initializing database: {}", config.system.database_path);
    let db = PositionDatabase::new(&config.system.database_path)?;

    // Perform crash recovery
    execution::persistence::recover_from_crash(&db).await?;

    // Check database state
    let open_positions = db.count_open_positions()?;
    tracing::info!("Open positions: {}", open_positions);

    tracing::info!("✅ Bot initialized successfully");
    tracing::info!("Waiting for trading signals...");

    // TODO: Start weather polling loop
    // TODO: Start strategy engine
    // TODO: Start WebSocket connection (if arbitrage enabled)

    // Keep running
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down...");

    Ok(())
}
