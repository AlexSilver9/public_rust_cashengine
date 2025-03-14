use std::hash::{Hash, Hasher};
use serde::{Serialize, Deserialize};
use crate::currency::Currency;
use crate::pair::Pair;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ring {
    pub r#type: i32,
    pub a_market_id: i32,
    pub b_market_id: i32,
    pub c_market_id: i32,
    pub a: Pair,
    pub b: Pair,
    pub c: Pair,
    pub as_string: String,
}

impl Ring {
    pub const TYPE_123: i32 = 1;    // B Q BQ
    pub const TYPE_142: i32 = 2;    // B QB Q
    pub const TYPE_214: i32 = 3;    // Q B QB
    pub const TYPE_231: i32 = 4;    // Q BQ B
    pub const TYPE_312: i32 = 5;    // BQ B Q
    pub const TYPE_421: i32 = 6;    // QB Q B

    pub const B: i32 = 1;
    pub const Q: i32 = 2;
    pub const BQ: i32 = 3;
    pub const QB: i32 = 4;

    pub fn new(a_market_id: i32, b_market_id: i32, c_market_id: i32, a: Pair, b: Pair, c: Pair, a_to_b: i32, b_to_c: i32, c_to_a: i32) -> Self {
        let r#type = match (a_to_b, b_to_c, c_to_a) {
            (Self::B, Self::Q, Self::BQ) => Self::TYPE_123,
            (Self::B, Self::QB, Self::Q) => Self::TYPE_142,
            (Self::Q, Self::B, Self::QB) => Self::TYPE_214,
            (Self::Q, Self::BQ, Self::B) => Self::TYPE_231,
            (Self::BQ, Self::B, Self::Q) => Self::TYPE_312,
            (Self::QB, Self::Q, Self::B) => Self::TYPE_421,
            _ => 0,
        };

        let as_string = format!("{} {} {} {}", r#type, a.as_string, b.as_string, c.as_string);

        Self {
            r#type,
            a_market_id,
            b_market_id,
            c_market_id,
            a,
            b,
            c,
            as_string,
        }
    }

    pub fn contains_currency(&self, currency: &Currency) -> bool {
        self.a.contains(currency) || self.b.contains(currency) || self.c.contains(currency)
    }

    pub fn contains_pair(&self, pair: &Pair) -> bool {
        self.a.as_string == pair.as_string || self.b.as_string == pair.as_string || self.c.as_string == pair.as_string
    }
}

impl Hash for Ring {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.r#type.hash(state);
        self.a_market_id.hash(state);
        self.b_market_id.hash(state);
        self.c_market_id.hash(state);
        self.a.hash(state);
        self.b.hash(state);
        self.c.hash(state);
    }
}

impl PartialEq for Ring {
    fn eq(&self, other: &Self) -> bool {
        self.r#type == other.r#type
            && self.a_market_id == other.a_market_id
            && self.b_market_id == other.b_market_id
            && self.c_market_id == other.c_market_id
            && self.a == other.a
            && self.b == other.b
            && self.c == other.c
    }
}

impl Eq for Ring {}

impl std::fmt::Display for Ring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\"Ring\":{{\"type\":\"{}\", \"aMarketId\":\"{}\", \"bMarketId\":\"{}\", \"cMarketId\":\"{}\", \"a\":{}, \"b\":{}, \"c\":{}, \"asString\":\"{}\"}}}}",
               self.r#type, self.a_market_id, self.b_market_id, self.c_market_id, self.a, self.b, self.c, self.as_string)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructor() {
        let a_pair = Pair::new("aBase".to_string(), "aQuote".to_string());
        let b_pair = Pair::new("bBase".to_string(), "bQuote".to_string());
        let c_pair = Pair::new("cBase".to_string(), "cQuote".to_string());
        let ring = Ring::new(1, 2, 3, a_pair.clone(), b_pair.clone(), c_pair.clone(), Ring::B, Ring::BQ, Ring::Q);

        assert_eq!(1, ring.a_market_id);
        assert_eq!(2, ring.b_market_id);
        assert_eq!(3, ring.c_market_id);
        assert_eq!(a_pair, ring.a);
        assert_eq!(b_pair, ring.b);
        assert_eq!(c_pair, ring.c);
    }

    #[test]
    fn test_contains_currency() {
        let a_pair = Pair::new("aBase".to_string(), "aQuote".to_string());
        let b_pair = Pair::new("bBase".to_string(), "bQuote".to_string());
        let c_pair = Pair::new("cBase".to_string(), "cQuote".to_string());
        let ring = Ring::new(1, 2, 3, a_pair, b_pair, c_pair, Ring::B, Ring::BQ, Ring::Q);

        assert!(ring.contains_currency(&Currency::new(5, 6, "aBase".to_string())));
        assert!(ring.contains_currency(&Currency::new(5, 6, "bBase".to_string())));
        assert!(ring.contains_currency(&Currency::new(5, 6, "bBase".to_string())));
        assert!(ring.contains_currency(&Currency::new(5, 6, "aQuote".to_string())));
        assert!(ring.contains_currency(&Currency::new(5, 6, "bQuote".to_string())));
        assert!(ring.contains_currency(&Currency::new(5, 6, "cQuote".to_string())));
        assert!(!ring.contains_currency(&Currency::new(5, 6, "d".to_string())));
    }

    #[test]
    fn test_contains_pair() {
        let a_pair = Pair::new("aBase".to_string(), "aQuote".to_string());
        let b_pair = Pair::new("bBase".to_string(), "bQuote".to_string());
        let c_pair = Pair::new("cBase".to_string(), "cQuote".to_string());
        let ring = Ring::new(1, 2, 3, a_pair.clone(), b_pair.clone(), c_pair.clone(), Ring::B, Ring::BQ, Ring::Q);

        assert!(ring.contains_pair(&a_pair.get_swap().get_swap()));
        assert!(ring.contains_pair(&b_pair.get_swap().get_swap()));
        assert!(ring.contains_pair(&c_pair.get_swap().get_swap()));
        assert!(!ring.contains_pair(&Pair::new("aBase".to_string(), "bQuote".to_string())));
        assert!(!ring.contains_pair(&Pair::new("c".to_string(), "d".to_string())));
    }

    #[test]
    fn test_to_string() {
        let a_pair = Pair::new("aBase".to_string(), "aQuote".to_string());
        let b_pair = Pair::new("bBase".to_string(), "bQuote".to_string());
        let c_pair = Pair::new("cBase".to_string(), "cQuote".to_string());
        let ring = Ring::new(1, 2, 3, a_pair, b_pair, c_pair, Ring::B, Ring::BQ, Ring::Q);

        println!("{}", ring);

        assert_eq!(
            ring.to_string(),
            "{\"Ring\":{\"type\":\"0\", \"aMarketId\":\"1\", \"bMarketId\":\"2\", \"cMarketId\":\"3\", \"a\":{\"Pair\":{\"base\":\"aBase\",\"quote\":\"aQuote\",\"asString\":\"aBase-aQuote\"}}, \"b\":{\"Pair\":{\"base\":\"bBase\",\"quote\":\"bQuote\",\"asString\":\"bBase-bQuote\"}}, \"c\":{\"Pair\":{\"base\":\"cBase\",\"quote\":\"cQuote\",\"asString\":\"cBase-cQuote\"}}, \"asString\":\"0 aBase-aQuote bBase-bQuote cBase-cQuote\"}}"
        );
    }

    #[test]
    fn test_equals_contract() {
        let a_pair1 = Pair::new("aBase".to_string(), "aQuote".to_string());
        let b_pair1 = Pair::new("bBase".to_string(), "bQuote".to_string());
        let c_pair1 = Pair::new("cBase".to_string(), "cQuote".to_string());
        let ring1 = Ring::new(1, 2, 3, a_pair1, b_pair1, c_pair1, Ring::B, Ring::BQ, Ring::Q);

        let a_pair2 = Pair::new("aBase".to_string(), "aQuote".to_string());
        let b_pair2 = Pair::new("bBase".to_string(), "bQuote".to_string());
        let c_pair2 = Pair::new("cBase".to_string(), "cQuote".to_string());
        let ring2 = Ring::new(1, 2, 3, a_pair2, b_pair2, c_pair2, Ring::B, Ring::BQ, Ring::Q);

        assert_eq!(ring1, ring2);
        assert_eq!(ring1.as_string, ring2.as_string);
    }
}