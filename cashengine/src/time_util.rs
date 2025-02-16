use std::time::{SystemTime, UNIX_EPOCH};

pub fn print_systemtime() {
    let current_system_time = SystemTime::now();
    match current_system_time.duration_since(UNIX_EPOCH) {
        Ok(duration_since_epoch) => {
            let milliseconds_timestamp = duration_since_epoch.as_millis();
            println!("Startup timestamp: {}", milliseconds_timestamp);
        },
        Err(err) => println!("Error getting duration for UNIX epoch: {}", err),
    }
}