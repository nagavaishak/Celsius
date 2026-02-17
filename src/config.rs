use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub system: SystemConfig,
    pub strategies: StrategiesConfig,
    pub risk: RiskConfig,
    pub infrastructure: InfrastructureConfig,
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub paper_trading: PaperTradingConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SystemConfig {
    pub dry_run: bool,
    pub database_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrategiesConfig {
    pub weather: WeatherStrategyConfig,
    pub arbitrage: ArbitrageStrategyConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeatherStrategyConfig {
    pub enabled: bool,
    pub min_edge: f64,
    pub target_cities: Vec<String>,
    pub forecast_lead_time_hours: u64,
    pub polling_interval_secs: u64,
    pub polling_interval_urgent_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArbitrageStrategyConfig {
    pub enabled: bool,
    pub min_spread: f64,
    pub min_spread_15min_crypto: f64,
    pub execution_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RiskConfig {
    pub max_position_size_usd: f64,
    pub max_position_pct: f64,
    pub max_open_positions: usize,
    pub max_daily_trades: usize,
    pub max_daily_loss_usd: f64,
    pub max_drawdown_pct: f64,
    pub max_positions_per_city_per_day: usize,
    pub claude_validation_weather: bool,
    pub claude_validation_arb: bool,
    pub min_liquidity_usd: f64,
    pub max_gas_gwei: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InfrastructureConfig {
    pub primary_rpc: String,
    pub secondary_rpc: String,
    pub rpc_timeout_secs: u64,
    pub rpc_failover_enabled: bool,
    pub websocket_reconnect_backoff_secs: u64,
    pub websocket_max_reconnect_delay_secs: u64,
    pub websocket_staleness_threshold_secs: u64,
    pub cache_ttl_arb_ms: u64,
    pub cache_ttl_weather_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringConfig {
    pub csv_logging: bool,
    pub csv_log_path: String,
    pub prometheus_enabled: bool,
    pub telegram_enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PaperTradingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_fill_rate")]
    pub fill_rate: f64,
    #[serde(default = "default_slippage")]
    pub slippage_pct: f64,
    #[serde(default = "default_balance")]
    pub initial_balance_usd: f64,
}

fn default_fill_rate() -> f64 { 0.70 }
fn default_slippage() -> f64 { 0.005 }
fn default_balance() -> f64 { 2000.0 }

#[derive(Debug, Clone)]
pub struct EnvConfig {
    pub polygon_rpc_primary: String,
    pub polygon_rpc_secondary: String,
    pub polygon_wallet_private_key: String,
    pub anthropic_api_key: String,
    pub noaa_api_key: Option<String>,
    pub polymarket_clob_url: String,
    pub polymarket_gamma_url: String,
    pub polymarket_ws_url: String,
    pub dry_run: bool,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path))?;
        
        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path))?;
        
        Ok(config)
    }
}

impl EnvConfig {
    pub fn load() -> Result<Self> {
        dotenv::dotenv().ok();
        
        Ok(Self {
            polygon_rpc_primary: std::env::var("POLYGON_RPC_PRIMARY")
                .context("POLYGON_RPC_PRIMARY not set")?,
            polygon_rpc_secondary: std::env::var("POLYGON_RPC_SECONDARY")
                .context("POLYGON_RPC_SECONDARY not set")?,
            polygon_wallet_private_key: std::env::var("POLYGON_WALLET_PRIVATE_KEY")
                .context("POLYGON_WALLET_PRIVATE_KEY not set")?,
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY")
                .context("ANTHROPIC_API_KEY not set")?,
            noaa_api_key: std::env::var("NOAA_API_KEY").ok(),
            polymarket_clob_url: std::env::var("POLYMARKET_CLOB_URL")
                .unwrap_or_else(|_| "https://clob.polymarket.com".to_string()),
            polymarket_gamma_url: std::env::var("POLYMARKET_GAMMA_URL")
                .unwrap_or_else(|_| "https://gamma-api.polymarket.com".to_string()),
            polymarket_ws_url: std::env::var("POLYMARKET_WS_URL")
                .unwrap_or_else(|_| "wss://ws-subscriptions-clob.polymarket.com/ws/".to_string()),
            dry_run: std::env::var("DRY_RUN")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        })
    }
}
