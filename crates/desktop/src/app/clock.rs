use std::time::{SystemTime, UNIX_EPOCH};

use remindr_core::domain::Timestamp;
use remindr_core::ports::Clock;

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
            });
        Timestamp::from_millis(millis)
    }
}
