use std::time::{SystemTime, UNIX_EPOCH};

pub fn print_systemtime() {
    let current_system_time = SystemTime::now();
    match current_system_time.duration_since(UNIX_EPOCH) {
        Ok(duration_since_epoch) => {
            let milliseconds_timestamp = duration_since_epoch.as_millis();
            tracing::info!("Startup timestamp: {}", milliseconds_timestamp);
        },
        Err(err) => tracing::error!("Error getting duration for UNIX epoch: {}", err),
    }
}