use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    pub min_amount: f64,
    pub max_amount: f64,
    pub min_price: f64,
    pub max_price: f64,
    pub min_order_value: f64, // minimum for price * amount
    pub use_min_notional_for_market_orders: bool,
}

impl Limits {
    pub fn new() -> Self {
        Self {
            min_amount: 0.0,
            max_amount: 0.0,
            min_price: 0.0,
            max_price: 0.0,
            min_order_value: 0.0,
            use_min_notional_for_market_orders: false,
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Limits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\"Limits\":{{\"minAmount\":\"{}\",\"maxAmount\":\"{}\",\"minPrice\":\"{}\",\"maxPrice\":\"{}\",\"minOrderValue\":\"{}\",\"useMinNotionalForMarketOrders\":\"{}\"}}}}",
               self.min_amount, self.max_amount, self.min_price, self.max_price, self.min_order_value, self.use_min_notional_for_market_orders)
    }
}