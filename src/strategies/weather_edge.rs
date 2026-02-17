use anyhow::Result;
use crate::config::WeatherStrategyConfig;
use crate::data::types::Market;
use crate::data::weather::WeatherClient;
use crate::data::gamma_api::{parse_weather_question, Comparison};
use crate::strategies::types::{Signal, Side, Strategy};
use tracing::{info, warn};

pub struct WeatherEdgeStrategy {
    config: WeatherStrategyConfig,
    weather_client: WeatherClient,
}

impl WeatherEdgeStrategy {
    pub fn new(config: WeatherStrategyConfig, weather_client: WeatherClient) -> Self {
        Self {
            config,
            weather_client,
        }
    }
    
    /// Analyze a weather market for trading opportunities
    /// This is the core strategy algorithm that combines:
    /// 1. NOAA probabilistic forecasts
    /// 2. Open-Meteo cross-validation
    /// 3. Edge calculation vs market price
    /// 4. Corrected Kelly position sizing
    pub async fn analyze_weather_market(
        &self,
        market: &Market,
        capital: f64,
        max_position_pct: f64,
    ) -> Result<Option<Signal>> {
        // 1. Parse market question
        let market_info = match parse_weather_question(&market.question) {
            Ok(info) => info,
            Err(e) => {
                warn!("Failed to parse market question: {} - {}", market.question, e);
                return Ok(None);
            }
        };
        
        info!(
            "Analyzing weather market: {} - threshold {}°C",
            market_info.city, market_info.threshold
        );
        
        // 2. Fetch NOAA probabilistic forecast
        let noaa_forecast = self.weather_client
            .fetch_probabilistic_forecast(&market_info.city, market_info.threshold)
            .await?;
        
        info!(
            "NOAA forecast: {:.1}% probability (mean={:.1}°C, std_dev={:.1}°C)",
            noaa_forecast.probability * 100.0,
            noaa_forecast.mean_temp,
            noaa_forecast.std_dev
        );
        
        // 3. Cross-validate with Open-Meteo
        let open_meteo_forecast = self.weather_client
            .fetch_open_meteo(&market_info.city, market_info.threshold)
            .await?;
        
        info!(
            "Open-Meteo forecast: {:.1}% probability",
            open_meteo_forecast.probability * 100.0
        );
        
        // Check forecast agreement (within 10%)
        let forecast_diff = (noaa_forecast.probability - open_meteo_forecast.probability).abs();
        if forecast_diff > 0.10 {
            warn!(
                "Forecast disagreement >10% ({:.1}%), skipping trade",
                forecast_diff * 100.0
            );
            return Ok(None);
        }
        
        // Use average of both forecasts
        let forecast_prob = (noaa_forecast.probability + open_meteo_forecast.probability) / 2.0;
        
        // Adjust for comparison type (above vs below)
        let forecast_prob_adjusted = match market_info.comparison {
            Comparison::Above => forecast_prob,
            Comparison::Below => 1.0 - forecast_prob,
        };
        
        // 4. Calculate edge
        let market_prob = market.yes_price;
        let edge = (forecast_prob_adjusted - market_prob).abs();
        
        info!(
            "Edge calculation: forecast={:.1}%, market={:.1}%, edge={:.1}%",
            forecast_prob_adjusted * 100.0,
            market_prob * 100.0,
            edge * 100.0
        );
        
        // 5. Check minimum edge threshold
        if edge < self.config.min_edge {
            info!(
                "Edge {:.1}% below minimum {:.1}%, skipping",
                edge * 100.0,
                self.config.min_edge * 100.0
            );
            return Ok(None);
        }
        
        // 6. Determine side (bet YES if forecast > market, NO otherwise)
        let side = if forecast_prob_adjusted > market_prob {
            Side::Yes
        } else {
            Side::No
        };
        
        let entry_price = match side {
            Side::Yes => market.yes_ask,
            Side::No => market.no_ask,
        };
        
        // 7. Calculate position size using CORRECTED Kelly
        let size = calculate_kelly_position(
            capital,
            forecast_prob_adjusted,
            entry_price,
            max_position_pct,
        );
        
        info!(
            "Signal generated: side={:?}, price=${:.2}, size=${:.2}, edge={:.1}%",
            side, entry_price, size, edge * 100.0
        );
        
        Ok(Some(Signal {
            market_id: market.id.clone(),
            strategy: Strategy::WeatherEdge,
            side: Some(side),
            entry_price,
            size,
            edge: Some(edge),
            confidence: (noaa_forecast.confidence + open_meteo_forecast.confidence) / 2.0,
        }))
    }
}

/// Calculate position size using CORRECTED Kelly Criterion
/// Formula: f* = (bp - q) / b
/// where b = odds, p = win_prob, q = lose_prob
pub fn calculate_kelly_position(
    capital: f64,
    forecast_prob: f64,
    market_price: f64,
    max_position_pct: f64,
) -> f64 {
    // Determine which side we're betting
    let (win_prob, bet_price) = if forecast_prob > market_price {
        (forecast_prob, market_price) // Bet YES
    } else {
        (1.0 - forecast_prob, 1.0 - market_price) // Bet NO
    };
    
    // Calculate odds: (1 - price) / price
    let odds = (1.0 - bet_price) / bet_price;
    
    // CORRECTED Kelly Criterion: f* = (bp - q) / b
    let lose_prob = 1.0 - win_prob;
    let kelly_fraction = (odds * win_prob - lose_prob) / odds;
    
    // Use 25% fractional Kelly for safety
    let fractional_kelly = kelly_fraction * 0.25;
    
    // Calculate position
    let position = capital * fractional_kelly.max(0.0); // No negative positions
    
    // Apply maximum position constraint
    let max_position = capital * max_position_pct;
    
    position.min(max_position)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_kelly_position_sizing() {
        // Example from spec:
        // Capital: $2,000
        // Forecast: 85% (0.85)
        // Market: $0.65
        //
        // Betting YES:
        // odds = (1.0 - 0.65) / 0.65 = 0.538
        // kelly = (0.538 * 0.85 - 0.15) / 0.538 = 0.570
        // fractional (25%): 0.570 * 0.25 = 0.1425
        // position = $2,000 * 0.1425 = $285
        // max_position = $2,000 * 0.10 = $200
        // FINAL: min($285, $200) = $200
        
        let size = calculate_kelly_position(2000.0, 0.85, 0.65, 0.10);
        assert!((size - 200.0).abs() < 1.0);
    }
    
    #[test]
    fn test_kelly_with_small_edge() {
        // Small edge should produce small position
        let size = calculate_kelly_position(2000.0, 0.52, 0.50, 0.10);
        assert!(size < 50.0);
    }
    
    #[test]
    fn test_kelly_with_large_edge() {
        // Large edge should hit max position constraint
        let size = calculate_kelly_position(2000.0, 0.95, 0.50, 0.10);
        assert!((size - 200.0).abs() < 1.0); // Should hit 10% max
    }
    
    #[test]
    fn test_kelly_betting_no() {
        // Forecast 20%, market 65% -> bet NO
        let size = calculate_kelly_position(2000.0, 0.20, 0.65, 0.10);
        assert!(size > 0.0); // Should generate valid position
    }
}
