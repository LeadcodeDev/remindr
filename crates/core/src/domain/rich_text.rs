use std::ops::Range;

use super::DomainError;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Link(String);

impl TryFrom<&str> for Link {
    type Error = DomainError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl Link {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextColor {
    Gray,
    Brown,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    Red,
    GrayBackground,
    BrownBackground,
    OrangeBackground,
    YellowBackground,
    GreenBackground,
    BlueBackground,
    PurpleBackground,
    PinkBackground,
    RedBackground,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Marks {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub code: bool,
    pub link: Option<Link>,
    pub color: Option<TextColor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub text: String,
    pub marks: Marks,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MarkChange {
    Bold(bool),
    Italic(bool),
    Underline(bool),
    Strikethrough(bool),
    Code(bool),
    Link(Option<Link>),
    Color(Option<TextColor>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RichText(Vec<Span>);

impl RichText {
    #[must_use]
    pub fn new(spans: impl IntoIterator<Item = Span>) -> RichText {
        todo!()
    }

    #[must_use]
    pub fn plain(text: &str) -> RichText {
        todo!()
    }

    #[must_use]
    pub fn spans(&self) -> &[Span] {
        &self.0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.iter().map(|span| span.text.len()).sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn to_plain(&self) -> String {
        self.0.iter().map(|span| span.text.as_str()).collect()
    }

    #[must_use]
    pub fn marks_at(&self, offset: usize) -> Marks {
        todo!()
    }

    pub fn split_at(&self, offset: usize) -> Result<(RichText, RichText), DomainError> {
        todo!()
    }

    #[must_use]
    pub fn concat(&self, other: &RichText) -> RichText {
        todo!()
    }

    pub fn insert(
        &self,
        offset: usize,
        text: &str,
        marks: &Marks,
    ) -> Result<RichText, DomainError> {
        todo!()
    }

    pub fn delete(&self, range: Range<usize>) -> Result<RichText, DomainError> {
        todo!()
    }

    pub fn apply_mark(
        &self,
        range: Range<usize>,
        change: &MarkChange,
    ) -> Result<RichText, DomainError> {
        todo!()
    }

    #[must_use]
    pub fn has_mark(&self, range: Range<usize>, change: &MarkChange) -> bool {
        todo!()
    }
}
