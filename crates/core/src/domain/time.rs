#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(i64);

impl Timestamp {
    #[must_use]
    pub fn from_millis(ms: i64) -> Self {
        Self(ms)
    }

    #[must_use]
    pub fn as_millis(self) -> i64 {
        self.0
    }
}
