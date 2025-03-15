use serde::{Deserialize, Serialize};

pub const PATH: &str = "/v1/settings/common/market-symbols";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct HtxMarket {

    #[serde(alias = "symbol")]
    pub symbol: Option<String>,  // false	symbol(outside)
    #[serde(alias = "bc")]
    pub base_currency: Option<String>,      // false	base currency
    #[serde(alias = "qc")]
    pub quote_currency: Option<String>,      // false	quote currency
    #[serde(alias = "state")]
    pub state: Option<String>, // false	symbol status. unknown，not-online，pre-online，online，suspend，offline，transfer-board，fuse
    #[serde(alias = "sp")]
    pub symbol_partition: Option<String>,    // false	symbol partition
    #[serde(alias = "tags")]
    pub tags: Option<String>,  // false	Tags, multiple tags are separated by commas, such as: st, hadax
    #[serde(alias = "lr")]
    pub leverage_ratio_of_margin_symbol: Option<f32>, // decimal false	leverage ratio of margin symbol, provided by Global
    #[serde(alias = "smlr")]
    pub leverage_ratio_of_super_margin_symbol: Option<f32>, //	decimal	false	leverage ratio of super-margin symbol, provided by Global
    #[serde(alias = "pp")]
    pub price_precision: Option<i32>,   //	integer	false	price precision
    #[serde(alias = "ap")]
    pub amount_precision: Option<i32>,   //	integer	false	amount precision
    #[serde(alias = "vp")]
    pub value_precision: Option<i32>,   //	integer	false	value precision
    #[serde(alias = "minoa")]
    pub min_order_amount: Option<f64>,  // 	decimal	false	min order amount
    #[serde(alias = "maxoa")]
    pub max_order_amount: Option<f64>,  //	decimal	false	max order amount
    #[serde(alias = "minov")]
    pub min_order_value: Option<f64>,   //	decimal	false	min order value
    #[serde(alias = "lominoa")]
    pub min_amount_of_limit_price_order: Option<f64>,   //	decimal	false	min amount of limit price order
    #[serde(alias = "lomaxoa")]
    pub max_amount_of_limit_price_order: Option<f64>,   //	decimal	false	max amount of limit price order
    #[serde(alias = "lomaxba")]
    pub max_amount_of_limit_price_buy_order: Option<f64>,   // decimal false max amount of limit price buy order
    #[serde(alias = "lomaxsa")]
    pub max_amount_of_limit_price_sell_order: Option<f64>,   // decimal false max amount of limit price sell order
    #[serde(alias = "smminoa")]
    pub min_amount_of_market_price_sell_order: Option<f64>,   // decimal false min amount of market price sell order
    #[serde(alias = "smmaxoa")]
    pub max_amount_of_market_price_sell_order: Option<f64>,   // decimal false max amount of market price sell order
    #[serde(alias = "bmmaxov")]
    pub max_amount_of_market_price_buy_order: Option<f64>,   // decimal false max amount of market price buy order
    #[serde(alias = "blmlt")]
    pub buy_limit_must_less_than: Option<f64>,   // Buy limit must less than
    #[serde(alias = "slmgt")]
    pub sell_limit_must_greater_than: Option<f64>,   // decimal(10,6) false Sell limit must greater than
    #[serde(alias = "msormlt")]
    pub market_sell_order_rate_must_less_than: Option<f64>,   // decimal(10,6) false Market sell order rate must less than
    #[serde(alias = "mbormlt")]
    pub market_buy_order_rate_must_less_than: Option<f64>,   // decimal(10,6) false Market buy order rate must less than
    #[serde(alias = "at")]
    pub trading_by_api_interface: Option<String>,   // string false trading by api interface
    #[serde(alias = "u")]
    pub etp_symbol: Option<String>,   // string false ETP: symbol
    #[serde(alias = "mfr")]
    pub unknown_mfr: Option<f64>,   // decimal false
    #[serde(alias = "ct")]
    pub charge_time: Option<u64>,   // string false charge time(unix time in millisecond, just for symbols of ETP)
    #[serde(alias = "rt")]
    pub rebal_time: Option<u64>,   // string false rebal time(unix time in millisecond, just for symbols of ETP)
    #[serde(alias = "rthr")]
    pub rebal_threshold: Option<f64>,   // decimal false rebal threshold(just for symbols of ETP)
    #[serde(alias = "in")]
    pub etp_init_nav: Option<f64>,   // decimal false ETP: init nav
    #[serde(alias = "maxov")]
    pub max_value_of_market_price_order: Option<f64>,   // decimal false max value of market price order
    #[serde(alias = "flr")]
    pub c2c_funding_leverage_ratio: Option<f64>,   // decimal false C2C: funding leverage ratio
    #[serde(alias = "castate")]
    pub castate: Option<String>,    //	string	false	not Required. The state of the call auction; it will only be displayed when it is in the 1st and 2nd stage of the call auction. Enumeration values: "ca_1", "ca_2"
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct HtxMarkets {
    pub status: Option<String>,           // false    status
    pub data: Vec<HtxMarket>,        // false    data
    #[serde(alias = "ts")]
    pub timestamp: String,               // false    timestamp of incremental data
    pub full: i8,                 // false    full data flag: 0 for no and 1 for yes
    #[serde(alias = "err-code")]
    pub err_code: Option<String>, // false	error code(returned when the interface reports an error)
    #[serde(alias = "err-msg")]
    pub err_msg: Option<String>, // false	error msg(returned when the interface reports an error)
}

impl HtxMarkets {
    // Parse symbols strong typed
    pub fn from(body: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(body)
    }

    fn filter<F>(&self, predicate: F) -> Self
    where
        F: Fn(&HtxMarket) -> bool,
    {
        let filtered_data: Vec<HtxMarket> =
            self.data.iter().filter(|m| predicate(m)).cloned().collect();

        Self {
            status: self.status.clone(),
            data: filtered_data,
            timestamp: self.timestamp.clone(),
            full: self.full,
            err_code: self.err_code.clone(),
            err_msg: self.err_msg.clone(),
        }
    }

    pub fn with_online_markets(&self) -> Self {
        self.filter(|m| m.state == Some("online".to_string()))
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn get_error(&self) -> Result<(), String> {
        if self.err_code.is_some() || self.err_msg.is_some() {
            let mut error_message = String::new();
            if let Some(code) = &self.err_code {
                error_message.push_str(code);
            }
            if let Some(msg) = &self.err_msg {
                if !error_message.is_empty() {
                    error_message.push_str(": ");
                }
                error_message.push_str(msg);
            }
            Err(error_message)
        } else {
            Ok(())
        }
    }
}
