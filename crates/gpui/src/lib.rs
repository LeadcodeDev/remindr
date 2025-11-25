use chrono::Utc;
use uuid::{NoContext, Timestamp, Uuid};

pub mod components;
pub mod entities;
pub mod screens;
pub mod states;

pub struct Utils;

impl Utils {
    pub fn generate_uuid() -> Uuid {
        let now = Utc::now();
        let seconds: u64 = now.timestamp().try_into().unwrap_or(0);
        let timestamp = Timestamp::from_unix(NoContext, seconds, 0);

        Uuid::new_v7(timestamp)
    }
}
