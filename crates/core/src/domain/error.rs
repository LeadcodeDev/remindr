use super::{BlockId, DatabaseId, OptionId, PropertyId, ViewId};

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum DomainError {
    #[error("unknown block {0:?}")]
    UnknownBlock(BlockId),
    #[error("unknown database {0:?}")]
    UnknownDatabase(DatabaseId),
    #[error("unknown property {0:?}")]
    UnknownProperty(PropertyId),
    #[error("unknown view {0:?}")]
    UnknownView(ViewId),
    #[error("unknown select option {0:?}")]
    UnknownOption(OptionId),
    #[error("offset {0} is outside the text or not on a char boundary")]
    InvalidOffset(usize),
    #[error("parent not allowed for this block")]
    InvalidParent,
    #[error("move would make a block its own ancestor")]
    CycleDetected,
    #[error("block kind does not accept children")]
    ChildrenNotAllowed,
    #[error("value does not match the property kind")]
    ValueKindMismatch,
    #[error("filter does not match the property kind")]
    ConditionKindMismatch,
    #[error("a database view block needs one view")]
    LastView,
    #[error("manual order is disabled while sorts are active")]
    ManualOrderWithSorts,
    #[error("invalid emoji")]
    InvalidEmoji,
    #[error("invalid link")]
    InvalidLink,
    #[error("invalid calendar date")]
    InvalidDate,
    #[error("invalid sort key")]
    InvalidSortKey,
    #[error("the title is the row page text")]
    TitleIsBlockText,
}
