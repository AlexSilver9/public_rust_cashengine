use std::hash::{Hash, Hasher};
use serde::{Serialize, Deserialize};
use crate::currency::Currency;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pair {
    pub base: String, // TODO: Use vec<u8> with fixed length
    pub quote: String,
    pub as_string: String, // TODO: Get rid of this field
}

impl Pair {
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        let base = base.into();
        let quote = quote.into();
        let as_string = format!("{}-{}", base, quote);

        Self {
            base,
            quote,
            as_string,
        }
    }

    pub fn contains(&self, currency: &Currency) -> bool {
        self.base == currency.name || self.quote == currency.name
    }

    pub fn get_swap(&self) -> Self {
        Self::new(&self.quote, &self.base)
    }
}

impl PartialEq for Pair {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.quote == other.quote
    }
}

impl Eq for Pair {}

impl Hash for Pair {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.base.hash(state);
        self.quote.hash(state);
    }
}

impl std::fmt::Display for Pair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\"Pair\":{{\"base\":\"{}\",\"quote\":\"{}\",\"asString\":\"{}\"}}}}",
               self.base, self.quote, self.as_string)
    }
}

#[cfg(test)]
mod tests {
    use super::Pair;

    #[test]
    fn test_as_string() {
        let pair = Pair::new("BTC", "ETH");
        let expected = "BTC-ETH";
        assert_eq!(expected, pair.as_string);
    }

    #[test]
    fn test_to_string() {
        let pair = Pair::new("BTC", "ETH");
        let expected = "{\"Pair\":{\"base\":\"BTC\",\"quote\":\"ETH\",\"asString\":\"BTC-ETH\"}}";
        assert_eq!(expected, pair.to_string());
    }

    #[test]
    fn test_equals() {
        let p = Pair::new("a", "b");
        assert_eq!(Pair::new("a", "b"), p);
        assert_ne!(Pair::new("a", "c"), p);
        assert_ne!(Pair::new("b", "b"), p);
    }
}