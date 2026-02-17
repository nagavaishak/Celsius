use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use crate::execution::types::{Position, Fill};
use crate::strategies::types::Side;

pub struct PositionDatabase {
    conn: Connection,
}

impl PositionDatabase {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        
        // Create tables
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS positions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                market_id TEXT NOT NULL,
                strategy TEXT NOT NULL,
                side TEXT,
                yes_shares REAL NOT NULL DEFAULT 0.0,
                no_shares REAL NOT NULL DEFAULT 0.0,
                entry_price REAL NOT NULL,
                cost REAL NOT NULL,
                opened_at TIMESTAMP NOT NULL,
                closed_at TIMESTAMP,
                pnl REAL,
                status TEXT NOT NULL DEFAULT 'open'
            );
            
            CREATE TABLE IF NOT EXISTS orders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                position_id INTEGER,
                market_id TEXT NOT NULL,
                side TEXT NOT NULL,
                token TEXT NOT NULL,
                price REAL NOT NULL,
                size REAL NOT NULL,
                order_type TEXT NOT NULL,
                submitted_at TIMESTAMP NOT NULL,
                filled_at TIMESTAMP,
                status TEXT NOT NULL DEFAULT 'pending',
                FOREIGN KEY(position_id) REFERENCES positions(id)
            );
            
            CREATE TABLE IF NOT EXISTS circuit_breaker_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                reason TEXT NOT NULL,
                triggered_at TIMESTAMP NOT NULL,
                reset_at TIMESTAMP,
                notes TEXT
            );
            
            CREATE TABLE IF NOT EXISTS emergency_exits (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                position_id INTEGER,
                reason TEXT NOT NULL,
                realized_loss REAL NOT NULL,
                exited_at TIMESTAMP NOT NULL,
                FOREIGN KEY(position_id) REFERENCES positions(id)
            );
            
            CREATE INDEX IF NOT EXISTS idx_positions_status ON positions(status);
            CREATE INDEX IF NOT EXISTS idx_positions_market_id ON positions(market_id);
            CREATE INDEX IF NOT EXISTS idx_positions_opened_at ON positions(opened_at);
            CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);
            "#
        )?;
        
        Ok(Self { conn })
    }
    
    /// Insert new position
    pub fn insert_position(&self, pos: &Position) -> Result<i64> {
        let side_str = pos.side.as_ref().map(|s| match s {
            Side::Yes => "YES",
            Side::No => "NO",
        });
        
        self.conn.execute(
            "INSERT INTO positions (market_id, strategy, side, yes_shares, no_shares, entry_price, cost, opened_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                pos.market_id,
                pos.strategy,
                side_str,
                pos.yes_shares,
                pos.no_shares,
                pos.entry_price,
                pos.cost,
                pos.opened_at.to_rfc3339(),
                pos.status,
            ],
        )?;
        
        Ok(self.conn.last_insert_rowid())
    }
    
    /// Get all open positions
    pub fn get_open_positions(&self) -> Result<Vec<Position>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, market_id, strategy, side, yes_shares, no_shares, entry_price, cost, opened_at, closed_at, pnl, status
             FROM positions
             WHERE status = 'open'"
        )?;
        
        let positions = stmt.query_map([], |row| {
            let side_str: Option<String> = row.get(3)?;
            let side = side_str.map(|s| if s == "YES" { Side::Yes } else { Side::No });
            
            let opened_at_str: String = row.get(8)?;
            let opened_at = DateTime::parse_from_rfc3339(&opened_at_str)
                .unwrap()
                .with_timezone(&Utc);
            
            let closed_at: Option<String> = row.get(9)?;
            let closed_at = closed_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));
            
            Ok(Position {
                id: Some(row.get(0)?),
                market_id: row.get(1)?,
                strategy: row.get(2)?,
                side,
                yes_shares: row.get(4)?,
                no_shares: row.get(5)?,
                entry_price: row.get(6)?,
                cost: row.get(7)?,
                opened_at,
                closed_at,
                pnl: row.get(10)?,
                status: row.get(11)?,
            })
        })?;

        positions.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }
    
    /// Count open positions
    pub fn count_open_positions(&self) -> Result<usize> {
        let count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM positions WHERE status = 'open'",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }
    
    /// Count positions for a specific city today (correlation check)
    pub fn count_positions_for_city_today(&self, city: &str) -> Result<usize> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        
        let count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM positions
             WHERE market_id LIKE ?1
             AND DATE(opened_at) = ?2
             AND status = 'open'",
            params![format!("%{}%", city), today],
            |row| row.get(0),
        )?;
        Ok(count)
    }
    
    /// Count trades today
    pub fn count_trades_today(&self) -> Result<usize> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        
        let count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM positions
             WHERE DATE(opened_at) = ?1",
            params![today],
            |row| row.get(0),
        )?;
        Ok(count)
    }
    
    /// Get daily P&L
    pub fn get_daily_pnl(&self) -> Result<f64> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        
        let pnl: Option<f64> = self.conn.query_row(
            "SELECT SUM(COALESCE(pnl, 0)) FROM positions
             WHERE DATE(opened_at) = ?1",
            params![today],
            |row| row.get(0),
        )?;
        
        Ok(pnl.unwrap_or(0.0))
    }
    
    /// Get peak equity
    pub fn get_peak_equity(&self) -> Result<f64> {
        // Calculate cumulative P&L and find peak
        let peak: Option<f64> = self.conn.query_row(
            "SELECT MAX(cumulative_pnl) FROM (
                SELECT SUM(COALESCE(pnl, 0)) OVER (ORDER BY opened_at) as cumulative_pnl
                FROM positions
                WHERE pnl IS NOT NULL
            )",
            [],
            |row| row.get(0),
        )?;
        
        Ok(peak.unwrap_or(0.0))
    }
    
    /// Update position status
    pub fn update_position_status(&self, id: i64, status: &str, pnl: Option<f64>) -> Result<()> {
        self.conn.execute(
            "UPDATE positions
             SET status = ?1, closed_at = ?2, pnl = ?3
             WHERE id = ?4",
            params![status, Utc::now().to_rfc3339(), pnl, id],
        )?;
        Ok(())
    }
    
    /// Update position shares (crash recovery reconciliation)
    pub fn update_position_shares(&self, id: i64, yes_shares: f64, no_shares: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE positions SET yes_shares = ?1, no_shares = ?2 WHERE id = ?3",
            params![yes_shares, no_shares, id],
        )?;
        Ok(())
    }
    
    /// Get pending orders
    pub fn get_pending_orders(&self) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, market_id FROM orders WHERE status = 'pending'"
        )?;
        
        let orders = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        orders.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }
    
    /// Mark order as filled
    pub fn mark_order_filled(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE orders SET status = 'filled', filled_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }
    
    /// Log circuit breaker event
    pub fn log_circuit_breaker_event(&self, reason: &str, notes: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO circuit_breaker_events (reason, triggered_at, notes)
             VALUES (?1, ?2, ?3)",
            params![reason, Utc::now().to_rfc3339(), notes],
        )?;
        Ok(())
    }
    
    /// Log emergency exit
    pub fn log_emergency_exit(
        &self,
        position_id: Option<i64>,
        reason: &str,
        realized_loss: f64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO emergency_exits (position_id, reason, realized_loss, exited_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![position_id, reason, realized_loss, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }
}

/// Crash recovery function
pub async fn recover_from_crash(db: &PositionDatabase) -> Result<()> {
    use tracing::{info, warn};
    
    info!("Performing crash recovery...");
    
    // Load open positions from SQLite
    let open_positions = db.get_open_positions()?;
    info!("Found {} open positions", open_positions.len());
    
    // TODO: Query on-chain state for each position
    // TODO: Reconcile SQLite vs on-chain balances
    // For now, just log what we found
    
    for pos in &open_positions {
        info!(
            "Open position: market={}, strategy={}, shares=({} YES, {} NO), cost=${}",
            pos.market_id, pos.strategy, pos.yes_shares, pos.no_shares, pos.cost
        );
    }
    
    // Check for pending orders
    let pending_orders = db.get_pending_orders()?;
    info!("Found {} pending orders", pending_orders.len());
    
    // TODO: Check order status via CLOB API
    
    info!("Crash recovery complete");
    Ok(())
}
