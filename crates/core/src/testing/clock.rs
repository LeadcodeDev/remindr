use std::cell::Cell;
use std::rc::Rc;

use crate::domain::Timestamp;
use crate::ports::Clock;

#[derive(Clone, Debug)]
pub struct ManualClock {
    now: Rc<Cell<i64>>,
}

impl ManualClock {
    #[must_use]
    pub fn new(start: Timestamp) -> Self {
        Self {
            now: Rc::new(Cell::new(start.as_millis())),
        }
    }

    pub fn advance(&self, ms: i64) {
        self.now.set(self.now.get() + ms);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(self.now.get())
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::Timestamp;
    use crate::ports::Clock;

    use super::ManualClock;

    #[test]
    fn clones_share_the_same_time() {
        let clock = ManualClock::new(Timestamp::from_millis(10));
        let handle = clock.clone();
        handle.advance(5);
        assert_eq!(clock.now(), Timestamp::from_millis(15));
    }
}
