use serde::{Serialize, Deserialize};
use strum_macros::Display;
use strum_macros::EnumString;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display)]
pub enum Type {
    None,
    FractionalDigits,
    SignificantDigits,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Precision {
    pub price: i32,
    pub price_precision_type: Type,
    pub amount: i32,
    pub amount_precision_type: Type,
}

impl Precision {
    pub fn new(price: i32, price_precision_type: Type, amount: i32, amount_precision_type: Type) -> Self {
        Self {
            price,
            price_precision_type,
            amount,
            amount_precision_type,
        }
    }
}

impl Default for Precision {
    fn default() -> Self {
        Self::new(0, Type::None, 0, Type::None)
    }
}

impl std::fmt::Display for Precision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\"Precision\":{{\"price\":\"{}\",\"pricePrecisionType\":\"{:?}\",\"amount\":\"{}\",\"amountPrecisionType\":\"{:?}\"}}}}",
               self.price, self.price_precision_type, self.amount, self.amount_precision_type)
    }
}