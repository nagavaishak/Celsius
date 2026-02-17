# Celsius - Polymarket Automated Trading Bot

Rust-based automated trading system for Polymarket prediction markets that exploits weather forecast divergences and sum-to-one arbitrage opportunities.

## Project Status

**Phase 0: Infrastructure Setup** ✅  
Core components implemented and ready for Phase 1 (Paper Trading)

## Architecture

- **Language:** Rust 1.75+
- **Strategy:** Weather market edge (primary), Sum-to-one arbitrage (Phase 3+)
- **Capital:** Progressive validation from $500 → $10K
- **ROI Target:** 5-15% monthly (conservative, Opus-reviewed)

## Quick Start

### Prerequisites

1. Rust 1.75+ installed
2. SQLite3
3. NOAA API access (free)
4. Polymarket account (for live trading)
5. Python 3.8+ (for thesis validation)

### Installation

```bash
# Clone repository
git clone https://github.com/nagavaishak/Celsius.git
cd Celsius

# Install dependencies
cargo build

# Set up environment variables
cp .env.example .env
# Edit .env with your API keys
```

### Phase 0: Thesis Validation (MANDATORY)

**CRITICAL:** Run this BEFORE any live trading to validate the edge exists:

```bash
python3 validate_thesis.py
```

This script validates:
- Average edge ≥5%
- Win rate ≥65%
- Opportunities ≥3/day

**If validation fails, DO NOT proceed** - there is no exploitable edge.

### Configuration

Edit `config.toml` for Phase 2 settings:
- `dry_run = true` for paper trading
- Risk limits: max position $50, max 2 positions
- Weather strategy enabled, arbitrage disabled

### Running the Bot

```bash
# Paper trading mode (dry_run=true)
cargo run

# Live trading (ONLY after successful validation)
# Edit config.toml: dry_run = false
cargo run
```

## Implementation Phases

### ✅ Phase 0: Infrastructure (Weeks 1-3)
- [x] SQLite persistence layer
- [x] NOAA + Open-Meteo weather client
- [x] Gamma API client
- [x] Weather edge strategy with corrected Kelly
- [x] Risk management (10-step validation)
- [x] Circuit breakers
- [x] DashMap cache with TTL
- [x] Paper trading simulator
- [x] CSV logging

### Phase 1: Paper Trading (Weeks 4-7, $0 risk)
30-day simulation testing edge thresholds (8%, 10%, 12%, 15%)

**Go/No-Go:** Net profit >$200 on $2K capital (>10% monthly)

### Phase 2: Live Testing (Weeks 8-9, $500 capital)
- Total: $500, Max position: $50, Max simultaneous: 2
- **Target:** -$50 to +$100 (break-even is success)

### Phase 3: Production (Month 3, $3K capital)
- Infrastructure: $228/month (VPS + RPCs + APIs)
- **Target:** $300-450/month (10-15% ROI)

### Phase 4: Scale (Month 4+, $10K capital)
- Infrastructure: $398/month
- **Target:** $1,000-1,500/month (10-15% ROI)

## Key Features

### Weather Edge Strategy (Primary)
- Fetches NOAA probabilistic forecasts
- Cross-validates with Open-Meteo
- Converts forecasts to probabilities using normal CDF
- **Corrected Kelly Criterion:** `f* = (bp - q) / b`
- 25% fractional Kelly for safety

### Risk Management
- 10-step pre-trade validation
- Circuit breakers (loss, drawdown, fill rate, latency)
- Max 10% position size, 15% drawdown limit
- Correlation limits (max 1 position per city/day)
- SQLite crash recovery

### Data Layer
- Strategy-aware cache TTL (500ms arb, 5min weather)
- Dual RPC failover (Alchemy + QuickNode)
- WebSocket reconnection with backoff
- Sequence number tracking

## Project Structure

```
src/
├── config.rs              # Configuration loading
├── data/                  # Data ingestion
│   ├── weather.rs         # NOAA + Open-Meteo client (THE PRODUCT)
│   ├── gamma_api.rs       # Polymarket market data
│   ├── cache.rs           # DashMap with TTL
│   └── types.rs
├── strategies/
│   ├── weather_edge.rs    # Probabilistic forecast model
│   └── types.rs
├── execution/
│   ├── persistence.rs     # SQLite position tracking
│   ├── risk.rs            # 10-step validation + circuit breakers
│   ├── simulator.rs       # Paper trading
│   └── types.rs
└── monitoring/
    └── logger.rs          # CSV trade logging (MANDATORY)
```

## Critical Implementation Notes

### Corrected Kelly Formula (Opus-Fixed)
```rust
// WRONG: let kelly = edge / odds;
// CORRECT:
let kelly = (odds * win_prob - lose_prob) / odds;
```

### Forecast-to-Probability Model
```rust
// P(temp > threshold) = 1 - CDF(threshold | N(mean, σ²))
let z_score = (threshold - mean_temp) / std_dev;
let probability = 1.0 - normal_cdf(z_score);
```

### Cache TTL
- Arbitrage: 500ms (speed critical)
- Weather: 5min (forecast stable)

### Claude AI Usage
- ✅ Weather markets: validation OK (24h+ resolution)
- ❌ Arbitrage: NEVER (kills latency)

## Testing

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test weather
cargo test kelly
cargo test cache
```

## Monitoring

All trades logged to CSV (mandatory):
```csv
timestamp,market_id,strategy,side,entry_price,size,cost,pnl,status
```

Check `trades.csv` after each session.

## Safety Features

- **Dry run mode:** Test without real money
- **Paper trading:** 70% fill rate, 0.5% slippage simulation
- **Circuit breakers:** Auto-stop on critical events
- **Crash recovery:** SQLite position persistence
- **Correlation limits:** Max 1 position per city/day
- **Drawdown protection:** 15% max from peak

## Documentation

See `CLAUDE.md` for comprehensive development guide including:
- Opus-reviewed patterns
- Common pitfalls
- Implementation priorities
- Phase success metrics

## License

MIT License - See LICENSE file

## Contributing

This is a personal trading bot. Not accepting external contributions.

## Disclaimer

**FOR EDUCATIONAL PURPOSES ONLY**

Trading prediction markets involves significant risk. This software is provided "as is" without warranty. Past performance does not guarantee future results. Only trade with capital you can afford to lose.

The authors are not responsible for any financial losses incurred through the use of this software.
