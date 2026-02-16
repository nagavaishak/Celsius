# Polymarket Automated Trading Bot - Development Guide

## Project Overview

Rust-based automated trading system for Polymarket prediction markets exploiting:
1. **Weather forecast divergences** (NOAA vs market prices) - PRIMARY STRATEGY
2. **Sum-to-one arbitrage** (YES + NO ≠ $1.00) - Phase 3+ only

**Capital:** $500 → $10K progressive validation
**Timeline:** 3-6 months
**Target ROI:** 5-15% monthly (conservative, Opus-reviewed)

## Tech Stack

- **Language:** Rust 1.75+
- **Async Runtime:** tokio 1.35 (full features)
- **Database:** SQLite (rusqlite 0.30) - position persistence + crash recovery
- **HTTP:** reqwest 0.11 (json features)
- **WebSocket:** tokio-tungstenite 0.21
- **Blockchain:** ethers 2.0 (Polygon mainnet)
- **Cache:** DashMap 5.5 (concurrent hashmap with TTL eviction)
- **Serialization:** serde 1.0, serde_json 1.0
- **Time:** chrono 0.4
- **Config:** dotenv 0.15, toml 0.8
- **Logging:** tracing 0.1, tracing-subscriber 0.3
- **Errors:** anyhow 1.0, thiserror 1.0

## Project Structure

```
polymarket-bot/
├── Cargo.toml
├── .env.example
├── config.toml              # Phase 2 configuration
├── README.md
├── validate_thesis.py       # PRE-RUST: 2-week validation script
│
├── src/
│   ├── main.rs              # Tokio runtime, weather polling loop
│   │
│   ├── data/                # Data ingestion layer
│   │   ├── mod.rs
│   │   ├── websocket.rs     # CLOB WebSocket + reconnect logic
│   │   ├── gamma_api.rs     # Polymarket Gamma API (market metadata)
│   │   ├── weather.rs       # NOAA + Open-Meteo (THE PRODUCT)
│   │   ├── cache.rs         # DashMap with strategy-aware TTL
│   │   └── types.rs
│   │
│   ├── strategies/          # Trading strategies
│   │   ├── mod.rs
│   │   ├── weather_edge.rs  # Forecast probability model (PRIMARY)
│   │   ├── sum_to_one.rs    # Arbitrage (Phase 3+ only)
│   │   └── types.rs
│   │
│   ├── execution/           # Order execution + risk management
│   │   ├── mod.rs
│   │   ├── clob_client.rs   # Dual RPC failover (Alchemy + QuickNode)
│   │   ├── order_manager.rs # Order lifecycle management
│   │   ├── risk.rs          # 10-step validation + circuit breakers
│   │   ├── simulator.rs     # Paper trading (70% fill, 0.5% slippage)
│   │   ├── persistence.rs   # SQLite position tracking
│   │   └── types.rs
│   │
│   ├── ai/                  # Claude AI integration
│   │   ├── mod.rs
│   │   ├── claude.rs        # WEATHER VALIDATION ONLY (not arb)
│   │   └── prompts.rs
│   │
│   └── monitoring/          # Observability
│       ├── mod.rs
│       ├── logger.rs        # CSV logs (MANDATORY)
│       ├── metrics.rs       # Prometheus (optional Phase 3+)
│       └── alerts.rs        # Telegram (optional Phase 3+)
│
├── backtest/                # Historical validation
│   ├── historical_prices.csv
│   ├── historical_forecasts.csv
│   └── backtest.rs
│
└── tests/
    └── integration_tests.rs
```

## Implementation Phases

### Phase 0: Python Validation (Week 1-2, $0 risk)
**CRITICAL:** Validate edge exists before building ANY Rust code

Build `validate_thesis.py`:
- Fetch NOAA forecasts for 14 days
- Fetch Polymarket prices for matching markets
- Log forecast vs price divergence
- **Success criteria:** Avg edge ≥5%, win rate ≥65%, ≥3 opportunities/day
- **IF FAIL → STOP PROJECT**

### Phase 1: Infrastructure (Week 3-5, $0 risk)
Build core Rust system:
- SQLite persistence layer (positions, orders, circuit breakers, emergency exits)
- NOAA + Open-Meteo probabilistic forecast client
- Gamma API client (market metadata)
- Weather edge detector with forecast-to-probability model
- Paper trading simulator
- CSV logging
- DashMap cache with TTL eviction
- Dual RPC client structure

**Success:** 7-day continuous run, logs 5-20 opportunities

### Phase 2: Paper Trading (Week 6-9, $0 risk)
30-day simulation with different edge thresholds (8%, 10%, 12%, 15%)

**Go/No-Go:** Net profit >$200 on $2K capital (>10% monthly)

### Phase 3: Live Testing (Week 10-11, $500 capital)
- Total: $500, Max position: $50, Max simultaneous: 2
- Local machine (Ireland), public RPCs
- **Target:** -$50 to +$100 (break-even is success)

### Phase 4: Production (Month 3, $3K capital)
- Infrastructure: $228/month (VPS $60 + Alchemy $99 + QuickNode $49 + Anthropic $20)
- **Target:** $300-450/month (10-15% ROI)

### Phase 5: Scale (Month 4+, $10K capital)
- Infrastructure: $398/month
- **Target:** $1,000-1,500/month (10-15% ROI)

## Core Algorithms (OPUS-CORRECTED)

### 1. Forecast-to-Probability Model (THE PRODUCT)

```rust
/// Convert NOAA point forecast to probability distribution
pub fn forecast_to_probability(
    point_forecast: f64,    // e.g., 16°C
    threshold: f64,          // e.g., 15°C
    uncertainty: Option<f64>, // NOAA uncertainty or default ±2.5°C
) -> f64 {
    let std_dev = uncertainty.unwrap_or(2.5);
    let z_score = (threshold - point_forecast) / std_dev;

    // P(temp > threshold) = 1 - CDF(z)
    1.0 - normal_cdf(z_score)
}

fn normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / f64::sqrt(2.0)))
}

fn erf(x: f64) -> f64 {
    // Abramowitz & Stegun approximation
    let a1 =  0.254829592;
    let a2 = -0.284496736;
    let a3 =  1.421413741;
    let a4 = -1.453152027;
    let a5 =  1.061405429;
    let p  =  0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5*t + a4)*t) + a3)*t + a2)*t + a1)*t * (-x*x).exp();

    sign * y
}
```

### 2. Kelly Criterion Position Sizing (OPUS-CORRECTED FORMULA)

```rust
/// CORRECTED Kelly: f* = (bp - q) / b
/// where b = odds, p = win_prob, q = lose_prob
pub fn calculate_kelly_position(
    capital: f64,
    forecast_prob: f64,  // e.g., 0.85
    market_price: f64,   // e.g., 0.65
) -> f64 {
    let (win_prob, bet_price) = if forecast_prob > market_price {
        (forecast_prob, market_price)  // Bet YES
    } else {
        (1.0 - forecast_prob, 1.0 - market_price)  // Bet NO
    };

    let odds = (1.0 - bet_price) / bet_price;
    let lose_prob = 1.0 - win_prob;
    let kelly_fraction = (odds * win_prob - lose_prob) / odds;

    // Use 25% fractional Kelly for safety
    let fractional_kelly = kelly_fraction * 0.25;
    let position = capital * fractional_kelly;

    // Apply 10% max position constraint
    position.min(capital * 0.10)
}
```

### 3. Weather Market Selection

```rust
pub fn should_trade_weather_market(market: &Market) -> bool {
    let allowed_cities = ["London", "New York", "Chicago", "Seoul"];
    let is_temperature = market.question.contains("temperature")
        || market.question.contains("°F")
        || market.question.contains("°C");
    let hours_until = (market.end_date - Utc::now()).num_hours();
    let has_liquidity = market.volume_24h > 5000.0;
    let clear_resolution = market.question.contains(">")
        || market.question.contains("<");

    is_temperature
        && hours_until >= 24
        && hours_until <= 72  // Max 3 days
        && has_liquidity
        && clear_resolution
}
```

## Critical Implementation Notes

### 1. NOAA Weather Client (Priority #1)

**This is the core value proposition. Build this FIRST.**

- Fetch probabilistic forecasts from NOAA API (hourly updates)
- Cross-validate with Open-Meteo (15-min updates for EU)
- Model temperature as normal distribution: N(mean, σ²)
- Calculate P(temp > threshold) using CDF
- Require forecast agreement within 10% (NOAA vs Open-Meteo)
- City-to-coordinates mapping for API calls

### 2. SQLite Persistence (Crash Recovery)

**Schema:**
```sql
CREATE TABLE positions (
    id INTEGER PRIMARY KEY,
    market_id TEXT NOT NULL,
    strategy TEXT NOT NULL,
    side TEXT,  -- 'YES', 'NO', or NULL for arb
    yes_shares REAL,
    no_shares REAL,
    entry_price REAL,
    cost REAL,
    opened_at TIMESTAMP,
    closed_at TIMESTAMP,
    pnl REAL,
    status TEXT  -- 'open', 'closed', 'legged'
);

CREATE TABLE orders (
    id INTEGER PRIMARY KEY,
    position_id INTEGER,
    market_id TEXT,
    side TEXT,
    token TEXT,  -- 'YES' or 'NO'
    price REAL,
    size REAL,
    order_type TEXT,  -- 'FOK', 'GTC'
    submitted_at TIMESTAMP,
    filled_at TIMESTAMP,
    status TEXT,  -- 'pending', 'filled', 'rejected'
    FOREIGN KEY(position_id) REFERENCES positions(id)
);

CREATE TABLE circuit_breaker_events (
    id INTEGER PRIMARY KEY,
    reason TEXT,
    triggered_at TIMESTAMP,
    reset_at TIMESTAMP,
    notes TEXT
);

CREATE TABLE emergency_exits (
    id INTEGER PRIMARY KEY,
    position_id INTEGER,
    reason TEXT,  -- 'legged_position', 'oracle_dispute'
    realized_loss REAL,
    exited_at TIMESTAMP,
    FOREIGN KEY(position_id) REFERENCES positions(id)
);
```

**Crash recovery:**
- Load open positions from SQLite on startup
- Query on-chain token balances for each position
- Reconcile SQLite vs on-chain state
- Check pending orders for fills

### 3. Risk Management (10-Step Validation)

**Pre-trade checklist:**
1. Capital check (sufficient USDC balance)
2. Position limits (max open positions not exceeded)
3. Daily trades (not exceeded max_daily_trades)
4. Daily loss (PnL > -max_daily_loss_usd)
5. Drawdown check (current equity > peak * (1 - max_drawdown_pct))
6. Market quality (liquidity > min_liquidity_usd)
7. Gas check (gas_price < max_gas_gwei)
8. Edge validation (edge not suspiciously high >30%)
9. Claude AI validation (weather only, not arb)
10. Correlation check (max 1 position per city per day)

### 4. Circuit Breakers

**Trigger reasons:**
- DailyLoss: >10% capital loss in one day
- Drawdown: >15% from peak equity
- FillRate: <40% over last 10 trades
- Latency: >5sec average execution
- ApiErrors: >10 errors in 1 hour
- LeggedPositionStuck: Emergency exit failed (CRITICAL)
- RpcFailure: Both RPCs down

**Recovery protocol:**
- Cancel all open orders
- Stop strategy engine
- Log event to SQLite
- Send emergency alert
- Require manual reset after cooldown period

### 5. DashMap Cache with Strategy-Aware TTL

```rust
pub struct CachedPrice {
    price: f64,
    timestamp: Instant,
    ttl: Duration,
}

impl PriceCache {
    pub fn insert(&self, key: String, price: f64, strategy: &str) {
        let ttl = match strategy {
            "sum_to_one_arb" => Duration::from_millis(500),  // 500ms
            "weather_edge" => Duration::from_secs(300),      // 5min
            _ => Duration::from_secs(60),
        };

        self.cache.insert(key, CachedPrice { price, timestamp: Instant::now(), ttl });
    }

    pub fn get(&self, key: &str) -> Option<f64> {
        self.cache.get(key).and_then(|entry| {
            if entry.timestamp.elapsed() > entry.ttl {
                None  // Stale, evict
            } else {
                Some(entry.price)
            }
        })
    }
}
```

### 6. Legged Position Handler (OPUS-SPECIFIED)

**Emergency exit procedure for arbitrage partial fills:**

```rust
async fn handle_legged_position(
    &mut self,
    yes_fill: Option<Fill>,
    no_fill: Option<Fill>,
) -> Result<()> {
    let (filled_side, fill) = match (yes_fill, no_fill) {
        (Some(f), None) => ("YES", f),
        (None, Some(f)) => ("NO", f),
        _ => return Ok(()),
    };

    // Market sell immediately, accept up to 5% loss
    let max_loss_pct = 0.05;
    let min_exit_price = fill.price * (1.0 - max_loss_pct);

    let exit_order = Order::fok(
        &fill.market_id,
        Side::Sell,
        if filled_side == "YES" { Token::Yes } else { Token::No },
        min_exit_price,
        fill.size,
    );

    // 1-second timeout
    match timeout(Duration::from_secs(1), self.clob_client.submit_order(exit_order)).await {
        Ok(Ok(exit_fill)) => {
            // Success - log loss and continue
            let realized_loss = fill.cost - exit_fill.proceeds;
            self.db.log_emergency_exit(&fill, &exit_fill, realized_loss)?;
            Ok(())
        }
        _ => {
            // CRITICAL: Exit failed, trigger circuit breaker
            self.circuit_breaker.trigger(CircuitBreakerReason::LeggedPositionStuck)?;
            self.alerts.send_critical("Manual intervention required").await?;
            Err(Error::LeggedPositionStuckCritical)
        }
    }
}
```

### 7. Weather Polling Frequency

```rust
pub async fn weather_polling_loop(&self) {
    loop {
        let markets = self.get_active_weather_markets().await?;

        for market in markets {
            let hours_until = (market.end_date - Utc::now()).num_hours();

            let poll_interval = if hours_until < 24 {
                Duration::from_secs(900)   // 15 minutes if urgent
            } else {
                Duration::from_secs(3600)  // 1 hour otherwise
            };

            let forecast = self.weather_client.fetch_forecast(&market).await?;
            self.cache.insert(market.id, forecast, poll_interval);
        }

        sleep(Duration::from_secs(900)).await;  // Check every 15 min
    }
}
```

## Configuration

**Phase 2 limits (config.toml):**
```toml
[system]
dry_run = true

[strategies.weather]
enabled = true
min_edge = 0.10  # Start at 10%, test 8%/12%/15%
target_cities = ["London", "New York", "Chicago", "Seoul"]
polling_interval_secs = 3600
polling_interval_urgent_secs = 900

[strategies.arbitrage]
enabled = false  # Phase 3+ only

[risk]
max_position_size_usd = 50.0
max_position_pct = 0.10
max_open_positions = 2
max_daily_trades = 5
max_daily_loss_usd = 50.0
max_drawdown_pct = 0.15
max_positions_per_city_per_day = 1  # Correlation limit

claude_validation_weather = true   # Weather only
claude_validation_arb = false      # Kills latency

[infrastructure]
primary_rpc = "alchemy"
secondary_rpc = "quicknode"
rpc_timeout_secs = 5
rpc_failover_enabled = true
```

## Common Pitfalls (OPUS-IDENTIFIED)

### ❌ WRONG: Original Kelly Formula
```rust
let kelly = edge / odds;  // INCORRECT
```

### ✅ CORRECT: Opus-Fixed Kelly Formula
```rust
let kelly = (odds * win_prob - lose_prob) / odds;  // CORRECT
```

---

### ❌ WRONG: Claude AI in Arbitrage Hot Path
```rust
if let Some(signal) = detect_arbitrage(&market).await? {
    let validated = claude.validate_signal(&signal).await?;  // KILLS LATENCY
    execute_arbitrage(signal).await?;
}
```

### ✅ CORRECT: Claude Only for Weather
```rust
// Arbitrage: No Claude validation
if let Some(signal) = detect_arbitrage(&market).await? {
    execute_arbitrage(signal).await?;  // <500ms required
}

// Weather: Claude validation OK (24h+ resolution)
if let Some(signal) = analyze_weather_market(&market).await? {
    let validated = claude.validate_weather_signal(&signal).await?;
    if validated {
        execute_weather_trade(signal).await?;
    }
}
```

---

### ❌ WRONG: No State Persistence
```rust
// Crash = lost position tracking, capital calculation wrong
let mut positions = Vec::new();  // In-memory only
```

### ✅ CORRECT: SQLite Persistence
```rust
let db = PositionDatabase::new("positions.db")?;
db.insert_position(&position)?;  // Survives crashes
```

---

### ❌ WRONG: Cache Without TTL
```rust
cache.insert(market_id, price);  // Stale data forever
```

### ✅ CORRECT: Strategy-Aware TTL
```rust
cache.insert(market_id, price, "sum_to_one_arb");  // 500ms TTL
cache.insert(market_id, price, "weather_edge");     // 5min TTL
```

---

### ❌ WRONG: Single RPC (No Failover)
```rust
let provider = Provider::new(&env::var("POLYGON_RPC")?);  // Single point of failure
```

### ✅ CORRECT: Dual RPC Failover
```rust
pub struct RpcClient {
    primary: Provider<Http>,
    secondary: Provider<Http>,
}

impl RpcClient {
    pub async fn call_with_failover<T>(&mut self, method: impl Fn(&Provider<Http>) -> T) -> Result<T> {
        match timeout(Duration::from_secs(5), method(&self.primary)).await {
            Ok(Ok(result)) => Ok(result),
            _ => {
                // Failover to secondary
                timeout(Duration::from_secs(5), method(&self.secondary)).await??
            }
        }
    }
}
```

## Development Workflow

1. **Always run `cargo check` before committing**
2. **Test with `dry_run=true` first**
3. **Review CSV logs after each session**
4. **Monitor SQLite for position drift**
5. **Check circuit breaker events daily**
6. **Validate forecast accuracy weekly**
7. **Rebalance capital allocation weekly (Phase 4+)**

## Success Metrics

### Phase 1 (Paper Trading)
- [ ] 30-day net profit >$200 on $2K capital
- [ ] Fill rate >60%
- [ ] Max drawdown <20%
- [ ] System uptime >95%

### Phase 2 (Live Testing)
- [ ] 2-week P&L: -$50 to +$100
- [ ] Fill rate >50%
- [ ] No critical bugs
- [ ] 2 consecutive profitable weeks

### Phase 3 (Production)
- [ ] 8 consecutive profitable weeks
- [ ] Monthly profit: $300-450 on $3K
- [ ] Sharpe >1.0
- [ ] Infrastructure cost <30% gross profit

## External Resources

- **Polymarket Gamma API:** https://gamma-api.polymarket.com
- **Polymarket CLOB API:** https://clob.polymarket.com
- **NOAA API:** https://api.weather.gov
- **Open-Meteo API:** https://open-meteo.com
- **Polygon RPC (Alchemy):** https://polygon-mainnet.g.alchemy.com
- **Polygon RPC (QuickNode):** https://quick-node-polygon.io

## Current Phase

**Phase 0: Infrastructure Setup**
- Building SQLite persistence layer
- Implementing NOAA weather client
- Creating paper trading simulator
- Setting up risk management framework

**Next milestone:** 7-day continuous paper trading run with logged opportunities
