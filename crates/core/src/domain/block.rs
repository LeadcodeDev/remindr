use super::{BlockId, DatabaseId, DomainError, RichText, SortKey, Timestamp};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Emoji(String);

impl TryFrom<&str> for Emoji {
    type Error = DomainError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl Emoji {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CodeLanguage(String);

impl CodeLanguage {
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self(name.to_owned())
    }

    #[must_use]
    pub fn plain_text() -> Self {
        Self::new("plain text")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Parent {
    Root,
    Block(BlockId),
    Database(DatabaseId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeadingLevel {
    One,
    Two,
    Three,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlockKind {
    Text,
    Heading(HeadingLevel),
    BulletedListItem,
    NumberedListItem,
    ToDo { checked: bool },
    Toggle,
    Quote,
    Callout { icon: Emoji },
    Code { language: CodeLanguage },
    Divider,
    Page { icon: Option<Emoji> },
    DatabaseView { database: DatabaseId },
}

impl BlockKind {
    #[must_use]
    pub fn accepts_children(&self) -> bool {
        matches!(
            self,
            Self::Text
                | Self::BulletedListItem
                | Self::NumberedListItem
                | Self::ToDo { .. }
                | Self::Toggle
                | Self::Quote
                | Self::Callout { .. }
                | Self::Page { .. }
        )
    }

    #[must_use]
    pub fn is_atomic(&self) -> bool {
        matches!(self, Self::Divider | Self::DatabaseView { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Block {
    pub id: BlockId,
    pub kind: BlockKind,
    pub parent: Parent,
    pub sort_key: SortKey,
    pub text: RichText,
    pub alive: bool,
    pub created_at: Timestamp,
}
