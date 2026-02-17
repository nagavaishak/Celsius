use anyhow::Result;
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;
use crate::execution::types::Position;

pub struct CsvLogger {
    log_path: String,
}

impl CsvLogger {
    pub fn new(log_path: String) -> Result<Self> {
        // Create CSV file with headers if it doesn't exist
        if !std::path::Path::new(&log_path).exists() {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(&log_path)?;
            
            writeln!(
                file,
                "timestamp,market_id,strategy,side,entry_price,size,cost,pnl,status"
            )?;
        }
        
        Ok(Self { log_path })
    }
    
    /// Log a position to CSV
    pub fn log_position(&self, position: &Position) -> Result<()> {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.log_path)?;
        
        let side_str = match &position.side {
            Some(side) => format!("{:?}", side),
            None => "BOTH".to_string(),
        };
        
        let pnl_str = match position.pnl {
            Some(pnl) => format!("{:.2}", pnl),
            None => "".to_string(),
        };
        
        writeln!(
            file,
            "{},{},{},{},{:.3},{:.2},{:.2},{},{}",
            position.opened_at.to_rfc3339(),
            position.market_id,
            position.strategy,
            side_str,
            position.entry_price,
            position.yes_shares + position.no_shares,
            position.cost,
            pnl_str,
            position.status
        )?;
        
        Ok(())
    }
    
    /// Log a trade event
    pub fn log_event(&self, event: &str) -> Result<()> {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.log_path)?;
        
        writeln!(
            file,
            "{},EVENT,{},,,,,,,",
            Utc::now().to_rfc3339(),
            event
        )?;
        
        Ok(())
    }
}
