use super::DomainError;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SortKey(String);

impl SortKey {
    #[must_use]
    pub fn between(before: Option<&SortKey>, after: Option<&SortKey>) -> SortKey {
        todo!()
    }

    pub fn parse(raw: &str) -> Result<SortKey, DomainError> {
        todo!()
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
