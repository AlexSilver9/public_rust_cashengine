use serde::{Serialize, Deserialize};
use crate::limits::Limits;
use crate::precision::Precision;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeeType {
    Percentage,
    Fixed,
    Circulated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Currency {
    pub id: i32,
    pub exchange_id: i32,
    pub name: String,

    pub deposit_enabled: bool,
    pub deposit_fee: f64,
    pub deposit_fee_type: FeeType,
    pub deposit_precision: Precision,
    pub deposit_limits: Limits,
    pub deposit_info: String,

    pub withdraw_enabled: bool,
    pub withdraw_fee: f64,
    pub withdraw_fee_type: FeeType,
    pub withdraw_precision: Precision,
    pub withdraw_limits: Limits,
    pub withdraw_info: String,
}

impl Currency {
    /// Constructs a new Currency object.
    pub fn new(id: i32, exchange_id: i32, name: String) -> Self {
        Self {
            id,
            exchange_id,
            name,
            deposit_enabled: false,
            deposit_fee: 0.0,
            deposit_fee_type: FeeType::Percentage,
            deposit_precision: Precision::default(),
            deposit_limits: Limits::default(),
            deposit_info: String::new(),
            withdraw_enabled: false,
            withdraw_fee: 0.0,
            withdraw_fee_type: FeeType::Percentage,
            withdraw_precision: Precision::default(),
            withdraw_limits: Limits::default(),
            withdraw_info: String::new(),
        }
    }
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\"Currency\":{{\"id\":\"{}\", \"exchangeId\":\"{}\", \"name\":\"{}\", \
               \"depositEnabled\":\"{}\", \"depositFee\":\"{}\", \"depositFeeType\":\"{:?}\", \
               \"depositPrecision\":{:?}, \"depositLimits\":{:?}, \"depositInfo\":{:?}, \
               \"withdrawEnabled\":\"{}\", \"withdrawFee\":\"{}\", \"withdrawFeeType\":\"{:?}\", \
               \"withdrawPrecision\":{:?}, \"withdrawLimits\":{:?}, \"withdrawInfo\":{:?}}}}}",
               self.id, self.exchange_id, self.name,
               self.deposit_enabled, self.deposit_fee, self.deposit_fee_type,
               self.deposit_precision, self.deposit_limits, self.deposit_info,
               self.withdraw_enabled, self.withdraw_fee, self.withdraw_fee_type,
               self.withdraw_precision, self.withdraw_limits, self.withdraw_info)
    }
}