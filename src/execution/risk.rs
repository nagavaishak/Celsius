use anyhow::{Context, Result};
use std::time::{Duration, SystemTime};
use crate::config::RiskConfig;
use crate::strategies::types::Signal;
use crate::execution::persistence::PositionDatabase;
use tracing::{error, warn, info};

#[derive(Debug, Clone)]
pub struct RiskManager {
    config: RiskConfig,
}

impl RiskManager {
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }
    
    /// Validate trade against all risk limits (10-step checklist)
    pub async fn validate_trade(
        &self,
        signal: &Signal,
        db: &PositionDatabase,
        current_balance: f64,
    ) -> Result<(), ValidationError> {
        // 1. Capital check
        if signal.size > current_balance {
            return Err(ValidationError::InsufficientBalance(signal.size, current_balance));
        }
        
        // 2. Position limits
        let open_count = db.count_open_positions()?;
        if open_count >= self.config.max_open_positions {
            return Err(ValidationError::MaxPositionsReached(open_count));
        }
        
        // 3. Daily trades
        let today_trades = db.count_trades_today()?;
        if today_trades >= self.config.max_daily_trades {
            return Err(ValidationError::DailyTradesExceeded(today_trades));
        }
        
        // 4. Daily loss
        let daily_pnl = db.get_daily_pnl()?;
        if daily_pnl < -self.config.max_daily_loss_usd {
            return Err(ValidationError::DailyLossLimitHit(daily_pnl));
        }
        
        // 5. Drawdown check
        let peak = db.get_peak_equity()?.max(current_balance);
        let drawdown = (peak - current_balance) / peak;
        if drawdown > self.config.max_drawdown_pct {
            return Err(ValidationError::DrawdownExceeded(drawdown));
        }
        
        // 6. Position size limits
        if signal.size > self.config.max_position_size_usd {
            return Err(ValidationError::PositionTooLarge(signal.size));
        }
        
        if signal.size > current_balance * self.config.max_position_pct {
            return Err(ValidationError::PositionExceedsPercentage(
                signal.size,
                current_balance * self.config.max_position_pct,
            ));
        }
        
        // 7. Edge validation (flag suspiciously high edges)
        if let Some(edge) = signal.edge {
            if edge > 0.30 {
                warn!("Edge >30% - potential data error");
                return Err(ValidationError::EdgeTooGoodToBeTrue(edge));
            }
        }
        
        // 8. Correlation check (weather markets only)
        // TODO: Extract city from market_id for correlation check
        // For now, skip this check
        
        // 9. Claude AI validation would go here
        // (implemented separately in strategy layer)
        
        // 10. All checks passed
        info!("Trade validation passed for signal: {:?}", signal.market_id);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Insufficient balance: need ${0:.2}, have ${1:.2}")]
    InsufficientBalance(f64, f64),
    
    #[error("Max positions reached: {0}")]
    MaxPositionsReached(usize),
    
    #[error("Daily trades exceeded: {0}")]
    DailyTradesExceeded(usize),
    
    #[error("Daily loss limit hit: ${0:.2}")]
    DailyLossLimitHit(f64),
    
    #[error("Drawdown exceeded: {0:.1}%")]
    DrawdownExceeded(f64),
    
    #[error("Position too large: ${0:.2}")]
    PositionTooLarge(f64),
    
    #[error("Position exceeds percentage: ${0:.2} > ${1:.2}")]
    PositionExceedsPercentage(f64, f64),
    
    #[error("Edge too good to be true: {0:.1}%")]
    EdgeTooGoodToBeTrue(f64),
    
    #[error("Correlation limit exceeded")]
    CorrelationLimitExceeded,
    
    #[error("Claude AI rejected signal")]
    ClaudeRejected,

    #[error("Database error: {0}")]
    DatabaseError(#[from] anyhow::Error),
}

/// Circuit breaker to stop all trading on critical events
pub struct CircuitBreaker {
    triggered: bool,
    reason: Option<CircuitBreakerReason>,
    trigger_time: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub enum CircuitBreakerReason {
    DailyLoss(f64),
    Drawdown(f64),
    FillRate(f64),
    Latency(Duration),
    ApiErrors(usize),
    LeggedPositionStuck,
    RpcFailure,
}

impl std::fmt::Display for CircuitBreakerReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerReason::DailyLoss(loss) => write!(f, "DailyLoss(${:.2})", loss),
            CircuitBreakerReason::Drawdown(dd) => write!(f, "Drawdown({:.1}%)", dd * 100.0),
            CircuitBreakerReason::FillRate(rate) => write!(f, "FillRate({:.1}%)", rate * 100.0),
            CircuitBreakerReason::Latency(dur) => write!(f, "Latency({:?})", dur),
            CircuitBreakerReason::ApiErrors(count) => write!(f, "ApiErrors({})", count),
            CircuitBreakerReason::LeggedPositionStuck => write!(f, "LeggedPositionStuck"),
            CircuitBreakerReason::RpcFailure => write!(f, "RpcFailure"),
        }
    }
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            triggered: false,
            reason: None,
            trigger_time: None,
        }
    }
    
    pub fn is_triggered(&self) -> bool {
        self.triggered
    }
    
    pub fn trigger(&mut self, reason: CircuitBreakerReason, db: &PositionDatabase) -> Result<()> {
        if self.triggered {
            return Ok(()); // Already triggered
        }
        
        error!("🔴 CIRCUIT BREAKER TRIGGERED: {}", reason);
        
        self.triggered = true;
        self.reason = Some(reason.clone());
        self.trigger_time = Some(SystemTime::now());
        
        // Log to database
        db.log_circuit_breaker_event(&reason.to_string(), None)?;
        
        Ok(())
    }
    
    pub fn can_reset(&self) -> Result<String, String> {
        if !self.triggered {
            return Ok("Circuit breaker not triggered".to_string());
        }
        
        let elapsed = self.trigger_time
            .unwrap()
            .elapsed()
            .unwrap_or(Duration::from_secs(0));
        
        match &self.reason {
            Some(CircuitBreakerReason::DailyLoss(_)) => {
                if elapsed < Duration::from_secs(86400) {
                    Err("Must wait 24h before reset".to_string())
                } else {
                    Ok("Manual review required".to_string())
                }
            }
            Some(CircuitBreakerReason::LeggedPositionStuck) => {
                Err("Manual confirmation required: Position closed via UI?".to_string())
            }
            Some(CircuitBreakerReason::RpcFailure) => {
                Ok("Test both RPCs, require both healthy".to_string())
            }
            _ => {
                if elapsed < Duration::from_secs(3600) {
                    Err(format!(
                        "Cooldown: {} minutes remaining",
                        (3600 - elapsed.as_secs()) / 60
                    ))
                } else {
                    Ok("Can reset".to_string())
                }
            }
        }
    }
    
    pub fn reset(&mut self) {
        info!("Circuit breaker reset");
        self.triggered = false;
        self.reason = None;
        self.trigger_time = None;
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}
