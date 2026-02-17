use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub id: String,
    pub question: String,
    pub end_date: DateTime<Utc>,
    pub yes_price: f64,
    pub yes_ask: f64,
    pub no_ask: f64,
    pub volume_24h: f64,
    pub yes_liquidity: f64,
    pub no_liquidity: f64,
}

#[derive(Debug, Clone)]
pub struct ProbabilisticForecast {
    pub probability: f64,
    pub confidence: f64,
    pub mean_temp: f64,
    pub std_dev: f64,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookUpdate {
    pub market_id: String,
    pub sequence: u64,
    pub yes_ask: f64,
    pub no_ask: f64,
    pub timestamp: DateTime<Utc>,
}
