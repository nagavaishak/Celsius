use chrono::{DateTime, Utc};
use crate::strategies::types::Side;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderType {
    FOK,  // Fill-or-Kill
    GTC,  // Good-til-Cancel
}

#[derive(Debug, Clone)]
pub struct Order {
    pub market_id: String,
    pub side: Side,
    pub token: Token,
    pub price: f64,
    pub size: f64,
    pub order_type: OrderType,
}

#[derive(Debug, Clone)]
pub struct Fill {
    pub market_id: String,
    pub size: f64,
    pub price: f64,
    pub cost: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub id: Option<i64>,
    pub market_id: String,
    pub strategy: String,
    pub side: Option<Side>,
    pub yes_shares: f64,
    pub no_shares: f64,
    pub entry_price: f64,
    pub cost: f64,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub pnl: Option<f64>,
    pub status: String,
}
