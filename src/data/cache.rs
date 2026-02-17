use dashmap::DashMap;
use std::time::{Duration, Instant};

pub struct PriceCache {
    cache: DashMap<String, CachedPrice>,
}

struct CachedPrice {
    price: f64,
    timestamp: Instant,
    ttl: Duration,
}

impl PriceCache {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }
    
    /// Insert price with strategy-aware TTL
    pub fn insert(&self, key: String, price: f64, strategy: &str) {
        let ttl = match strategy {
            "sum_to_one_arb" => Duration::from_millis(500), // 500ms for arb
            "weather_edge" => Duration::from_secs(300),      // 5min for weather
            _ => Duration::from_secs(60),
        };
        
        self.cache.insert(key, CachedPrice {
            price,
            timestamp: Instant::now(),
            ttl,
        });
    }
    
    /// Get price if not expired (evict on read)
    pub fn get(&self, key: &str) -> Option<f64> {
        self.cache.get(key).and_then(|entry| {
            // Check if expired
            if entry.timestamp.elapsed() > entry.ttl {
                drop(entry); // Drop the read lock
                self.cache.remove(key); // Evict stale entry
                None
            } else {
                Some(entry.price)
            }
        })
    }
    
    /// Clear all entries
    pub fn clear(&self) {
        self.cache.clear();
    }
    
    /// Get cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }
    
    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for PriceCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_cache_insert_and_get() {
        let cache = PriceCache::new();
        cache.insert("BTC".to_string(), 50000.0, "weather_edge");
        
        assert_eq!(cache.get("BTC"), Some(50000.0));
    }
    
    #[test]
    fn test_cache_ttl_expiration() {
        let cache = PriceCache::new();
        cache.insert("ETH".to_string(), 3000.0, "sum_to_one_arb"); // 500ms TTL
        
        // Should exist immediately
        assert_eq!(cache.get("ETH"), Some(3000.0));
        
        // Wait for expiration
        thread::sleep(Duration::from_millis(600));
        
        // Should be evicted
        assert_eq!(cache.get("ETH"), None);
    }
    
    #[test]
    fn test_different_ttls() {
        let cache = PriceCache::new();
        
        // Arb: 500ms TTL
        cache.insert("ARB".to_string(), 1.0, "sum_to_one_arb");
        
        // Weather: 5min TTL
        cache.insert("WEATHER".to_string(), 2.0, "weather_edge");
        
        thread::sleep(Duration::from_millis(600));
        
        // Arb should be expired
        assert_eq!(cache.get("ARB"), None);
        
        // Weather should still be valid
        assert_eq!(cache.get("WEATHER"), Some(2.0));
    }
}
