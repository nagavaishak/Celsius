use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use chrono::{DateTime, Utc};
use crate::data::types::Market;

pub struct GammaApiClient {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct GammaMarket {
    #[allow(dead_code)]
    condition_id: String,
    question: String,
    #[serde(default)]
    end_date_iso: Option<String>,
    #[serde(default)]
    closed: bool,
    #[allow(dead_code)]
    description: Option<String>,
    #[allow(dead_code)]
    market_slug: Option<String>,
    #[serde(default)]
    volume: Option<String>,
    #[serde(default)]
    liquidity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GammaMarketsResponse {
    #[serde(default)]
    data: Vec<GammaMarket>,
}

impl GammaApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }
    
    /// Fetch all active markets from Polymarket Gamma API
    pub async fn fetch_markets(&self) -> Result<Vec<Market>> {
        let url = format!("{}/markets", self.base_url);
        
        let response: GammaMarketsResponse = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch markets")?
            .json()
            .await
            .context("Failed to parse markets response")?;
        
        let markets: Vec<Market> = response.data
            .into_iter()
            .filter_map(|gm| self.convert_gamma_market(gm).ok())
            .collect();
        
        Ok(markets)
    }
    
    /// Fetch weather markets specifically
    pub async fn fetch_weather_markets(&self) -> Result<Vec<Market>> {
        let all_markets = self.fetch_markets().await?;
        
        Ok(all_markets
            .into_iter()
            .filter(|m| self.is_weather_market(m))
            .collect())
    }
    
    /// Convert Gamma API market format to our internal Market type
    fn convert_gamma_market(&self, gm: GammaMarket) -> Result<Market> {
        let end_date = gm.end_date_iso
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| Utc::now() + chrono::Duration::days(7));
        
        let volume_24h = gm.volume
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let liquidity = gm.liquidity
            .and_then(|l| l.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        Ok(Market {
            id: gm.condition_id.clone(),
            question: gm.question,
            end_date,
            yes_price: 0.5, // Default, will be updated from order book
            yes_ask: 0.5,
            no_ask: 0.5,
            volume_24h,
            yes_liquidity: liquidity / 2.0,
            no_liquidity: liquidity / 2.0,
        })
    }
    
    /// Check if market is a weather market
    fn is_weather_market(&self, market: &Market) -> bool {
        let question_lower = market.question.to_lowercase();
        
        // Weather keywords
        let has_weather_keyword = question_lower.contains("temperature")
            || question_lower.contains("temp")
            || question_lower.contains("°f")
            || question_lower.contains("°c")
            || question_lower.contains("degrees")
            || question_lower.contains("weather")
            || question_lower.contains("rain")
            || question_lower.contains("snow");
        
        // Target cities
        let target_cities = ["london", "new york", "nyc", "chicago", "seoul"];
        let has_target_city = target_cities.iter()
            .any(|city| question_lower.contains(city));
        
        has_weather_keyword && has_target_city
    }
}

/// Check if we should trade this weather market
pub fn should_trade_weather_market(market: &Market, config_cities: &[String]) -> bool {
    let question_lower = market.question.to_lowercase();
    
    // Must be temperature market (highest accuracy)
    let is_temperature = question_lower.contains("temperature")
        || question_lower.contains("temp")
        || question_lower.contains("°f")
        || question_lower.contains("°c");
    
    if !is_temperature {
        return false;
    }
    
    // Must be in target cities
    let in_target_city = config_cities.iter()
        .any(|city| question_lower.contains(&city.to_lowercase()));
    
    if !in_target_city {
        return false;
    }
    
    // Minimum lead time (24h for forecast reliability)
    let hours_until_resolution = (market.end_date - Utc::now()).num_hours();
    if hours_until_resolution < 24 || hours_until_resolution > 72 {
        return false; // Max 3 days (forecast degrades)
    }
    
    // Minimum liquidity
    if market.volume_24h < 5000.0 {
        return false;
    }
    
    // Clear resolution criteria
    let has_clear_threshold = question_lower.contains(">")
        || question_lower.contains("<")
        || question_lower.contains("above")
        || question_lower.contains("below")
        || question_lower.contains("exceed");
    
    if !has_clear_threshold {
        return false;
    }
    
    true
}

/// Parse market question to extract city, date, threshold, and comparison
pub fn parse_weather_question(question: &str) -> Result<WeatherMarketInfo> {
    // Example: "Will NYC temperature exceed 60°F on 2026-02-17?"
    
    let question_lower = question.to_lowercase();
    
    // Extract city
    let city = if question_lower.contains("london") {
        "London"
    } else if question_lower.contains("new york") || question_lower.contains("nyc") {
        "New York"
    } else if question_lower.contains("chicago") {
        "Chicago"
    } else if question_lower.contains("seoul") {
        "Seoul"
    } else {
        anyhow::bail!("Could not identify city in question")
    };
    
    // Extract threshold
    let threshold = extract_temperature(question)?;
    
    // Extract comparison type
    let comparison = if question_lower.contains("exceed")
        || question_lower.contains("above")
        || question_lower.contains(">") {
        Comparison::Above
    } else if question_lower.contains("below")
        || question_lower.contains("<") {
        Comparison::Below
    } else {
        anyhow::bail!("Could not identify comparison type")
    };
    
    Ok(WeatherMarketInfo {
        city: city.to_string(),
        threshold,
        comparison,
    })
}

fn extract_temperature(question: &str) -> Result<f64> {
    // Look for patterns like "60°F", "15°C", "60 degrees"
    let re = regex::Regex::new(r"(\d+(?:\.\d+)?)\s*(?:°[FC]|degrees?)")?;
    
    if let Some(cap) = re.captures(question) {
        let temp = cap[1].parse::<f64>()?;
        
        // Convert to Celsius if Fahrenheit
        let temp_celsius = if question.contains("°F") || question.contains("degrees F") {
            (temp - 32.0) * 5.0 / 9.0
        } else {
            temp
        };
        
        Ok(temp_celsius)
    } else {
        anyhow::bail!("Could not extract temperature from question")
    }
}

#[derive(Debug, Clone)]
pub struct WeatherMarketInfo {
    pub city: String,
    pub threshold: f64,
    pub comparison: Comparison,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comparison {
    Above,
    Below,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_weather_question() {
        let question = "Will NYC temperature exceed 60°F on 2026-02-17?";
        let info = parse_weather_question(question).unwrap();
        
        assert_eq!(info.city, "New York");
        assert!((info.threshold - 15.56).abs() < 0.1); // 60°F ≈ 15.56°C
        assert_eq!(info.comparison, Comparison::Above);
    }
    
    #[test]
    fn test_extract_temperature() {
        assert!((extract_temperature("60°F").unwrap() - 15.56).abs() < 0.1);
        assert!((extract_temperature("15°C").unwrap() - 15.0).abs() < 0.1);
        assert!((extract_temperature("20.5 degrees C").unwrap() - 20.5).abs() < 0.1);
    }
}
