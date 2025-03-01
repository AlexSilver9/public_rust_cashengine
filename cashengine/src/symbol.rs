use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;
use std::io::Write;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Symbol {
    // field, type, required, description
    pub symbol: Option<String>,  // false	symbol(outside)
    pub sn: Option<String>,      // false	symbol name
    pub bc: Option<String>,      // false	base currency
    pub qc: Option<String>,      // false	quote currency
    pub state: Option<String>, // false	symbol status. unknown，not-online，pre-online，online，suspend，offline，transfer-board，fuse
    pub ve: Option<bool>,      // false	visible
    pub we: Option<bool>,      // false	white enabled
    pub dl: Option<bool>,      // false	delist
    pub cd: Option<bool>,      // false	country disabled
    pub te: Option<bool>,      // false	trade enabled
    pub ce: Option<bool>,      // false	cancel enabled
    pub tet: Option<u64>,      // false	trade enable timestamp
    pub toa: Option<u64>,      // false	the time trade open at
    pub tca: Option<u64>,      // false	the time trade close at
    pub voa: Option<u64>,      // false	visible open at
    pub vca: Option<u64>,      // false	visible close at
    pub sp: Option<String>,    // false	symbol partition
    pub tm: Option<String>,    // false	symbol partition
    pub w: Option<u64>,        // false	weight sort
    pub ttp: Option<f64>,      // false	trade total precision -> decimal(10,6)
    pub tap: Option<f64>,      // false	trade amount precision -> decimal(10,6)
    pub tpp: Option<f64>,      // false	trade price precision -> decimal(10,6)
    pub fp: Option<f64>,       // false	fee precision -> decimal(10,6)
    pub tags: Option<String>,  // false	Tags, multiple tags are separated by commas, such as: st, hadax
    pub d: Option<String>,     // false
    pub bcdn: Option<String>,  // false	base currency display name
    pub qcdn: Option<String>,  // false	quote currency display name
    pub elr: Option<String>,   // false	etp leverage ratio
    pub castate: Option<String>, // false	Not required. The state of the call auction; it will only be displayed when it is in the 1st and 2nd stage of the call auction. Enumeration values: "ca_1", "ca_2"
    pub ca1oa: Option<u64>, // false	not Required. the open time of call auction phase 1, total milliseconds since January 1, 1970 0:0:0:00ms UTC
    pub ca1ca: Option<u64>, // false	not Required. the close time of call auction phase 1, total milliseconds since January 1, 1970 0:0:0:00ms UTC
    pub ca2oa: Option<u64>, // false	not Required. the open time of call auction phase 2, total milliseconds since January 1, 1970 0:0:0:00ms UTC
    pub ca2ca: Option<u64>, // false	not Required. the close time of call auction phase 2, total milliseconds since January 1, 1970 0:0:0:00ms UTC
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Symbols {
    pub status: String,           // false    status
    pub data: Vec<Symbol>,        // false    data
    pub ts: String,               // false    timestamp of incremental data
    pub full: i8,                 // false    full data flag: 0 for no and 1 for yes
    pub err_code: Option<String>, // false	error code(returned when the interface reports an error)  -> err-code -> TODO: parse this manually because it is no underscore
    pub err_msg: Option<String>, // false	error msg(returned when the interface reports an error)  -> err-code -> TODO: parse this manually because it is no underscore
}

impl Symbols {
    // Parse symbols strong typed
    pub fn from(body: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(body)
    }

    pub fn from_as_map(body: &str) -> Result<Map<String, Value>, Box<dyn std::error::Error>> {
        let symbols_response: Map<String, Value> = serde_json::from_str(body)?;

        // Print non-data fields
        symbols_response
            .iter()
            .filter(|(key, _)| *key != "data")
            .for_each(|(key, value)| println!("Key: {}, Value: {:?}", key, value));

        if let Some(data) = symbols_response.get("data").and_then(Value::as_array) {
            let symbols: Vec<_> = data
                .iter()
                .filter_map(|item| item.as_object())
                .map(|item_map| {
                    let symbol = item_map
                        .get("symbol")
                        .and_then(Value::as_str)
                        .unwrap_or("N/A");
                    let base = item_map.get("bc").and_then(Value::as_str).unwrap_or("N/A");
                    let quote = item_map.get("qc").and_then(Value::as_str).unwrap_or("N/A");
                    (symbol, base, quote)
                })
                .collect();

            symbols.iter().for_each(|(symbol, base, quote)| {
                println!("Symbol: {}, Base: {}, Quote: {}", symbol, base, quote);
            });

            println!("Symbols count: {}", symbols.len());
        } else {
            println!("No 'data' field found or it's not an array");
        }
        Ok(symbols_response)
    }

    fn filter_symbols<F>(&self, predicate: F) -> Self
    where
        F: Fn(&Symbol) -> bool,
    {
        let filtered_data: Vec<Symbol> =
            self.data.iter().filter(|s| predicate(s)).cloned().collect();

        Self {
            status: self.status.clone(),
            data: filtered_data,
            ts: self.ts.clone(),
            full: self.full,
            err_code: self.err_code.clone(),
            err_msg: self.err_msg.clone(),
        }
    }

    pub fn with_online_symbols(&self) -> Self {
        self.filter_symbols(|s| s.state == Some("online".to_string()))
    }

    pub fn with_trade_enabled_symbols(&self) -> Self {
        self.filter_symbols(|s| s.te == Some(true))
    }

    pub fn with_cancel_enabled_symbols(&self) -> Self {
        self.filter_symbols(|s| s.ce == Some(true))
    }

    pub fn with_visible_symbols(&self) -> Self {
        self.filter_symbols(|s| s.ve == Some(true))
    }

    pub fn with_listed_symbols(&self) -> Self {
        self.filter_symbols(|s| s.dl == Some(false))
    }

    pub fn with_country_enabled_symbols(&self) -> Self {
        self.filter_symbols(|s| s.cd == Some(false))
    }

    pub fn get_symbols(&self) -> Vec<&Symbol> {
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

    pub fn print(&self) {
        self.print_common_info();
        self.print_symbols(|index, symbol| SymbolPrinter(index, symbol).to_string());
    }

    pub fn print_compact(&self) {
        self.print_common_info();
        self.print_symbols(|index, symbol| CompactSymbolPrinter(index, symbol).to_string());
    }

    fn print_common_info(&self) {
        println!("Symbols Status: {}", self.status);
        println!("Timestamp: {}", self.ts);
        println!("Full Data: {}", if self.full == 1 { "Yes" } else { "No" });

        if let Some(err_code) = &self.err_code {
            println!("Error Code: {}", err_code);
        }
        if let Some(err_msg) = &self.err_msg {
            println!("Error Message: {}", err_msg);
        }
    }

    fn print_symbols<F>(&self, printer: F)
    where
        F: Fn(usize, &Symbol) -> String,
    {
        println!("\nSymbols:");
        for (index, symbol) in self.data.iter().enumerate() {
            println!("{}", printer(index + 1, symbol));
        }
        println!("Total Symbols: {}", self.data.len());
    }

    pub fn print_custom<F>(&self, custom_printer: F)
    where
        F: Fn(usize, &Symbol) -> String,
    {
        self.print_common_info();
        self.print_symbols(custom_printer);
    }
}

struct SymbolPrinter<'a>(usize, &'a Symbol);

impl<'a> fmt::Display for SymbolPrinter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (index, symbol) = (self.0, self.1);
        write!(f, "{}. Symbol: {}", index, symbol.symbol.as_deref().unwrap_or("N/A"))?;
        write!(f, ", Base: {}", symbol.bc.as_deref().unwrap_or("N/A"))?;
        write!(f, ", Quote: {}", symbol.qc.as_deref().unwrap_or("N/A"))?;
        write!(f, ", State: {}", symbol.state.as_deref().unwrap_or("N/A"))?;
        write!(f, ", Trade Enabled: {}", symbol.te.unwrap_or(false))?;
        if let Some(tet) = symbol.tet {
            write!(f, ", Trade Enable Timestamp: {}", tet)?;
        }
        write!(f, ", Symbol Partition: {}", symbol.sp.as_deref().unwrap_or("N/A"))?;
        if let Some(tags) = &symbol.tags {
            write!(f, " Tags: {}", tags)?;
        }
        // TODO: Implement remaining fields
        Ok(())
    }
}

struct CompactSymbolPrinter<'a>(usize, &'a Symbol);

impl<'a> fmt::Display for CompactSymbolPrinter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (index, symbol) = (self.0, self.1);
        write!(f, "{}. Symbol: {}", index, symbol.symbol.as_deref().unwrap_or("N/A"))?;
        write!(f, ", Base: {}", symbol.bc.as_deref().unwrap_or("N/A"))?;
        write!(f, ", Quote: {}", symbol.qc.as_deref().unwrap_or("N/A"))?;
        write!(f, ", State: {}", symbol.state.as_deref().unwrap_or("N/A"))?;
        write!(f, ", Weight Sort: {}", symbol.w.map_or("N/A".to_string(), |w| w.to_string()))?;
        write!(f, ", Trade Total Precision: {}", symbol.ttp.map_or("N/A".to_string(), |ttp| ttp.to_string()) )?;
        write!(f, ", Trade Amount Precision: {}", symbol.tap.map_or("N/A".to_string(), |tap| tap.to_string()))?;
        write!(f, ", Trade Price Precision: {}", symbol.tpp.map_or("N/A".to_string(), |tpp| tpp.to_string()))?;
        write!(f, ", Fee Precision: {}", symbol.fp.map_or("N/A".to_string(), |fp| fp.to_string()))?;
        Ok(())
    }
}
