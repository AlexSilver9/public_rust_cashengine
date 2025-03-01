use serde::de::Error;
use serde::{de, Deserialize, Serialize};

pub const PATH: &str = "/v2/settings/common/currencies";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Currency {
    #[serde(alias = "cc")]
    pub currency_code: Option<String>,               // cc	string	false	currency code
    #[serde(alias = "dn")]
    pub currency_display: Option<String>,                // dn	string	false	currency display name
    #[serde(alias = "fn")]
    pub currency_full_name: Option<String>,               // fn	string	false	currency full name
    #[serde(alias = "at")]
    pub asset_type: Option<i32>,              // at	int	false	asset type, 1 virtual currency 2 fiat currency
    #[serde(alias = "wp")]
    pub withdraw_precision: Option<i32>,              // wp	int	false	withdraw precision
    #[serde(alias = "ft")]
    pub fee_type: Option<String>,                // ft	string	false	fee type, eth: Fixed fee, btc: Interval fee husd: Fee charged in proportion
    #[serde(alias = "dma")]
    pub deposit_min_amount: Option<String>,                 // dma	string	false	deposit min amount
    #[serde(alias = "wma")]
    pub withdraw_min_amount: Option<String>,                // wma	string	false	withdraw min amount
    #[serde(alias = "sp")]
    pub show_precision: Option<String>,              // sp	string	false	show precision
    #[serde(alias = "w")]
    pub weight: Option<i32>,                               // w	string	fw	string	false
    #[serde(alias = "qc", deserialize_with = "de_from_str")]
    pub be_quote_currency: Option<String>,                //     // qc	boolean	false	be quote currency
    #[serde(alias = "state")]
    pub state: Option<String>,                // state	string	false	symbol state. unkown, not-online, online, offline
    #[serde(alias = "v")]
    pub visible_or_not: Option<bool>,              // v	boolean	false	visible or not -- users who have offline currency but have assets can see it
    #[serde(alias = "whe")]
    pub white_enabled: Option<bool>,               // whe	boolean	false	white enabled
    #[serde(alias = "cd")]
    pub country_disabled: Option<bool>,                // cd	boolean	false	country disabled--users who have country disabled currency but have assets can see it
    #[serde(alias = "de")]
    pub deposit_enabled: Option<bool>,             // de	boolean	false	deposit enabled
    #[serde(alias = "wed")]
    pub withdraw_enabled: Option<bool>,                // wed	boolean	false	withdraw enabled
    #[serde(alias = "cawt")]
    pub currency_addr_with_tag: Option<bool>,               // cawt	boolean	false	currency addr with tag
    #[serde(alias = "fc")]
    pub fast_confirms: Option<i32>,               // fc	int	false	fast confirms
    #[serde(alias = "sc")]
    pub safe_confirms: Option<i32>,               // sc	int	false	safe confirms
    #[serde(alias = "swd")]
    pub suspend_withdraw: Option<String>,                // swd	string	false	suspend withdraw desc
    #[serde(alias = "wd")]
    pub withdraw_desc: Option<String>,               // wd	string	false	withdraw desc
    #[serde(alias = "sdd")]
    pub suspend_deposit: Option<String>,                 // sdd	string	false	suspend deposit desc
    #[serde(alias = "dd")]
    pub deposit_desc: Option<String>,                // dd	string	false	deposit desc
    #[serde(alias = "svd")]
    pub suspend_visible_desc: Option<String>,                 // svd	string	false	suspend visible desc
    #[serde(alias = "tags")]
    pub tags: Option<String>,               // tags	string	false	Tags, multiple tags are separated by commas, such as: st, hadax
    #[serde(alias = "tap")]
    pub undocumented: Option<i8>,              // undocumented
}

fn de_from_str<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: de::Deserializer<'de>
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => Ok(Some(s)),
        serde_json::Value::Bool(b) => Ok(Some(b.to_string())),
        _ => Err(D::Error::custom("Failed to deserialize to String or bool")),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Currencies {
    // Parameter	Data Type	Required	Description
    pub status: String,     // false	status
    pub data: Vec<Currency>,     // false	data
    #[serde(alias = "ts")]
    pub timestamp: String,	// false	timestamp of incremental data
    pub full: i8,	                // false	full data flag: 0 for no and 1 for yes
    #[serde(alias = "err-code")]
    pub err_code: Option<String>, // false	error code(returned when the interface reports an error)
    #[serde(alias = "err-msg")]
    pub err_msg: Option<String>, // false	error msg(returned when the interface reports an error)
}

impl Currencies {
    pub fn from(body: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(body)
    }

    fn filter<F>(&self, predicate: F) -> Self
    where
        F: Fn(&Currency) -> bool,
    {
        let filtered_data: Vec<Currency> =
            self.data.iter().filter(|c| predicate(c)).cloned().collect();

        Self {
            status: self.status.clone(),
            data: filtered_data,
            timestamp: self.timestamp.clone(),
            full: self.full,
            err_code: self.err_code.clone(),
            err_msg: self.err_msg.clone(),
        }
    }

    pub fn with_online_currencies(&self) -> Self {
        self.filter(|c| c.state == Some("online".to_string()))
    }

    pub fn with_country_enabled(&self) -> Self {
        self.filter(|c| c.country_disabled == Some(false))
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn get_currencies(&self) -> Vec<&Currency> {
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
}

