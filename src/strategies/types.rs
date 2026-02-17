#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Side {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Strategy {
    WeatherEdge,
    SumToOneArb,
}

#[derive(Debug, Clone)]
pub struct Signal {
    pub market_id: String,
    pub strategy: Strategy,
    pub side: Option<Side>,
    pub entry_price: f64,
    pub size: f64,
    pub edge: Option<f64>,
    pub confidence: f64,
}
