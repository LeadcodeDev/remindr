use crate::domain::Timestamp;
use crate::ports::Clock;

#[derive(Debug)]
pub struct ManualClock;

impl ManualClock {
    #[must_use]
    pub fn new(start: Timestamp) -> Self {
        todo!()
    }

    pub fn advance(&self, ms: i64) {
        todo!()
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        todo!()
    }
}
