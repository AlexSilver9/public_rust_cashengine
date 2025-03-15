use serde::{Deserialize, Serialize};
use std::fmt;

pub const PATH: &str = "/v1/settings/common/symbols";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct HtxSymbol {
    // field, type, required, description
    #[serde(alias = "symbol")]
    pub symbol: Option<String>,  // false	symbol(outside)
    #[serde(alias = "sn")]
    pub symbol_name: Option<String>,      // false	symbol name
    #[serde(alias = "bc")]
    pub base_currency: Option<String>,      // false	base currency
    #[serde(alias = "qc")]
    pub quote_currency: Option<String>,      // false	quote currency
    #[serde(alias = "state")]
    pub state: Option<String>, // false	symbol status. unknown，not-online，pre-online，online，suspend，offline，transfer-board，fuse
    #[serde(alias = "ve")]
    pub visible: Option<bool>,      // false	visible
    #[serde(alias = "we")]
    pub white_enabled: Option<bool>,      // false	white enabled
    #[serde(alias = "dl")]
    pub delist: Option<bool>,      // false	delist
    #[serde(alias = "cd")]
    pub country_disabled: Option<bool>,      // false	country disabled
    #[serde(alias = "te")]
    pub trade_enabled: Option<bool>,      // false	trade enabled
    #[serde(alias = "ce")]
    pub cancel_enabled: Option<bool>,      // false	cancel enabled
    #[serde(alias = "tet")]
    pub trade_enable_timestamp: Option<u64>,      // false	trade enable timestamp
    #[serde(alias = "toa")]
    pub time_trade_open_at: Option<u64>,      // false	the time trade open at
    #[serde(alias = "tca")]
    pub time_trade_close_at: Option<u64>,      // false	the time trade close at
    #[serde(alias = "voa")]
    pub visible_open_at: Option<u64>,      // false	visible open at
    #[serde(alias = "vca")]
    pub visible_close_at: Option<u64>,      // false	visible close at
    #[serde(alias = "sp")]
    pub symbol_partition: Option<String>,    // false	symbol partition
    #[serde(alias = "tm")]
    pub trade_maker_aka_symbol_partition: Option<String>,    // false	symbol partition (??) -> doc wrong, value PRO seems like TradeMaker
    #[serde(alias = "w")]
    pub weight_sort: Option<u64>,        // false	weight sort
    #[serde(alias = "ttp")]
    pub trade_total_precision: Option<f64>,      // false	trade total precision -> decimal(10,6)
    #[serde(alias = "tap")]
    pub trade_amount_precision: Option<f64>,      // false	trade amount precision -> decimal(10,6)
    #[serde(alias = "tpp")]
    pub trade_price_precision: Option<f64>,      // false	trade price precision -> decimal(10,6)
    #[serde(alias = "fp")]
    pub fee_precision: Option<f64>,       // false	fee precision -> decimal(10,6)
    #[serde(alias = "tags")]
    pub tags: Option<String>,  // false	Tags, multiple tags are separated by commas, such as: st, hadax
    #[serde(alias = "d")]
    pub unknown_d: Option<String>,     // false
    #[serde(alias = "bcdn")]
    pub base_currency_display_name: Option<String>,  // false	base currency display name
    #[serde(alias = "qcdn")]
    pub quote_currency_display_name: Option<String>,  // false	quote currency display name
    #[serde(alias = "elr")]
    pub etp_leverage_ratio: Option<String>,   // false	etp leverage ratio
    #[serde(alias = "castate")]
    pub call_auction_state: Option<String>, // false	Not required. The state of the call auction; it will only be displayed when it is in the 1st and 2nd stage of the call auction. Enumeration values: "ca_1", "ca_2"
    #[serde(alias = "ca1oa")]
    pub open_time_of_call_auction_phase_1: Option<u64>, // false	not Required. the open time of call auction phase 1, total milliseconds since January 1, 1970 0:0:0:00ms UTC
    #[serde(alias = "ca1ca")]
    pub close_time_of_call_auction_phase_1: Option<u64>, // false	not Required. the close time of call auction phase 1, total milliseconds since January 1, 1970 0:0:0:00ms UTC
    #[serde(alias = "ca2oa")]
    pub open_time_of_call_auction_phase_2: Option<u64>, // false	not Required. the open time of call auction phase 2, total milliseconds since January 1, 1970 0:0:0:00ms UTC
    #[serde(alias = "ca2ca")]
    pub close_time_of_call_auction_phase_2: Option<u64>, // false	not Required. the close time of call auction phase 2, total milliseconds since January 1, 1970 0:0:0:00ms UTC
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct HtxSymbols {
    pub status: String,           // false    status
    pub data: Vec<HtxSymbol>,        // false    data
    #[serde(alias = "ts")]
    pub timestamp: String,               // false    timestamp of incremental data
    pub full: i8,                 // false    full data flag: 0 for no and 1 for yes
    #[serde(alias = "err-code")]
    pub err_code: Option<String>, // false	error code(returned when the interface reports an error)
    #[serde(alias = "err-msg")]
    pub err_msg: Option<String>, // false	error msg(returned when the interface reports an error)
}

impl HtxSymbols {
    // Parse symbols strong typed
    pub fn from(body: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(body)
    }

    fn filter<F>(&self, predicate: F) -> Self
    where
        F: Fn(&HtxSymbol) -> bool,
    {
        let filtered_data: Vec<HtxSymbol> =
            self.data.iter().filter(|s| predicate(s)).cloned().collect();

        Self {
            status: self.status.clone(),
            data: filtered_data,
            timestamp: self.timestamp.clone(),
            full: self.full,
            err_code: self.err_code.clone(),
            err_msg: self.err_msg.clone(),
        }
    }

    pub fn with_online_symbols(&self) -> Self {
        self.filter(|s| s.state == Some("online".to_string()))
    }

    pub fn with_trade_enabled_symbols(&self) -> Self {
        self.filter(|s| s.trade_enabled == Some(true))
    }

    pub fn with_cancel_enabled_symbols(&self) -> Self {
        self.filter(|s| s.cancel_enabled == Some(true))
    }

    pub fn with_visible_symbols(&self) -> Self {
        self.filter(|s| s.visible == Some(true))
    }

    pub fn with_listed_symbols(&self) -> Self {
        self.filter(|s| s.delist == Some(false))
    }

    pub fn with_country_enabled(&self) -> Self {
        self.filter(|s| s.country_disabled == Some(false))
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn get_symbols(&self) -> Vec<&HtxSymbol> {
        self.data.iter().collect()
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

    pub fn log_compact(&self) {
        self.log_common_info();
        self.log_symbols(|index, symbol| CompactSymbolPrinter(index, symbol).to_string());
    }

    fn log_common_info(&self) {
        tracing::info!("Symbols Status: {}", self.status);
        tracing::info!("Timestamp: {}", self.timestamp);
        tracing::debug!("Full Data: {}", if self.full == 1 { "Yes" } else { "No" });

        if let Some(err_code) = &self.err_code {
            tracing::error!("Error Code: {}", err_code);
        }
        if let Some(err_msg) = &self.err_msg {
            tracing::error!("Error Message: {}", err_msg);
        }
    }

    fn log_symbols<F>(&self, printer: F)
    where
        F: Fn(usize, &HtxSymbol) -> String,
    {
        tracing::debug!("\nSymbols:");
        for (index, symbol) in self.data.iter().enumerate() {
            tracing::debug!("{}", printer(index + 1, symbol));
        }
        tracing::debug!("Total Symbols: {}", self.data.len());
    }
}

struct CompactSymbolPrinter<'a>(usize, &'a HtxSymbol);

impl<'a> fmt::Display for CompactSymbolPrinter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (index, symbol) = (self.0, self.1);
        write!(f, "{}. Symbol: {}", index, symbol.symbol.as_deref().unwrap_or("N/A"))?;
        write!(f, ", Base: {}", symbol.base_currency.as_deref().unwrap_or("N/A"))?;
        write!(f, ", Quote: {}", symbol.quote_currency.as_deref().unwrap_or("N/A"))?;
        write!(f, ", State: {}", symbol.state.as_deref().unwrap_or("N/A"))?;
        write!(f, ", Weight Sort: {}", symbol.weight_sort.map_or("N/A".to_string(), |w| w.to_string()))?;
        write!(f, ", Trade Total Precision: {}", symbol.trade_total_precision.map_or("N/A".to_string(), |ttp| ttp.to_string()) )?;
        write!(f, ", Trade Amount Precision: {}", symbol.trade_amount_precision.map_or("N/A".to_string(), |tap| tap.to_string()))?;
        write!(f, ", Trade Price Precision: {}", symbol.trade_price_precision.map_or("N/A".to_string(), |tpp| tpp.to_string()))?;
        write!(f, ", Fee Precision: {}", symbol.fee_precision.map_or("N/A".to_string(), |fp| fp.to_string()))?;
        Ok(())
    }
}
