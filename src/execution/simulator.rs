use anyhow::Result;
use chrono::Utc;
use rand::Rng;
use crate::execution::types::{Order, Fill, Position};
use crate::config::PaperTradingConfig;
use crate::strategies::types::Side;
use tracing::info;

pub struct PaperTradingSimulator {
    config: PaperTradingConfig,
    balance: f64,
}

impl PaperTradingSimulator {
    pub fn new(config: PaperTradingConfig) -> Self {
        let balance = config.initial_balance_usd;
        info!("Paper trading simulator initialized with ${:.2}", balance);
        
        Self {
            config,
            balance,
        }
    }
    
    /// Simulate order execution
    pub fn execute_order(&mut self, order: &Order) -> Result<Option<Fill>> {
        // Simulate fill rate (70% by default)
        let mut rng = rand::thread_rng();
        let will_fill = rng.gen::<f64>() < self.config.fill_rate;
        
        if !will_fill {
            info!("Order not filled (simulated rejection)");
            return Ok(None);
        }
        
        // Apply simulated slippage
        let slippage = rng.gen::<f64>() * self.config.slippage_pct;
        let executed_price = order.price * (1.0 + slippage);
        
        let cost = order.size * executed_price;
        
        // Check balance
        if cost > self.balance {
            info!("Insufficient balance for order");
            return Ok(None);
        }
        
        // Deduct from balance
        self.balance -= cost;
        
        info!(
            "Order filled: {:?} {} shares @ ${:.3} (slippage: {:.2}%)",
            order.token,
            order.size,
            executed_price,
            slippage * 100.0
        );
        
        Ok(Some(Fill {
            market_id: order.market_id.clone(),
            size: order.size,
            price: executed_price,
            cost,
            timestamp: Utc::now(),
        }))
    }
    
    /// Get current balance
    pub fn balance(&self) -> f64 {
        self.balance
    }
    
    /// Add to balance (simulate winnings)
    pub fn add_to_balance(&mut self, amount: f64) {
        self.balance += amount;
    }
    
    /// Create simulated position from fills
    pub fn create_position_from_fill(
        &self,
        fill: &Fill,
        side: Side,
        strategy: &str,
    ) -> Position {
        let (yes_shares, no_shares) = match side {
            Side::Yes => (fill.size, 0.0),
            Side::No => (0.0, fill.size),
        };
        
        Position {
            id: None,
            market_id: fill.market_id.clone(),
            strategy: strategy.to_string(),
            side: Some(side),
            yes_shares,
            no_shares,
            entry_price: fill.price,
            cost: fill.cost,
            opened_at: fill.timestamp,
            closed_at: None,
            pnl: None,
            status: "open".to_string(),
        }
    }
}
