use std::ops::Range;

use super::DomainError;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Link(String);

impl TryFrom<&str> for Link {
    type Error = DomainError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
            return Err(DomainError::InvalidLink);
        }
        Ok(Link(trimmed.to_owned()))
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
        let mut normalized: Vec<Span> = Vec::new();
        for span in spans.into_iter().filter(|span| !span.text.is_empty()) {
            match normalized.last_mut() {
                Some(last) if last.marks == span.marks => last.text.push_str(&span.text),
                _ => normalized.push(span),
            }
        }
        RichText(normalized)
    }

    #[must_use]
    pub fn plain(text: &str) -> RichText {
        RichText::new([Span {
            text: text.to_owned(),
            marks: Marks::default(),
        }])
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

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{Link, Marks, RichText, Span};

    fn span() -> impl Strategy<Value = Span> {
        ("[a-zé ]{0,4}", any::<bool>(), any::<bool>()).prop_map(|(text, bold, italic)| Span {
            text,
            marks: Marks {
                bold,
                italic,
                ..Marks::default()
            },
        })
    }

    #[test]
    fn plain_empty_text_has_no_span() {
        assert!(RichText::plain("").spans().is_empty());
    }

    #[test]
    fn link_rejects_blank_and_inner_whitespace() {
        assert!(Link::try_from("  ").is_err());
        assert!(Link::try_from("https://a b").is_err());
        assert_eq!(
            Link::try_from(" https://x.dev ").unwrap().as_str(),
            "https://x.dev"
        );
    }

    proptest! {
        #[test]
        fn new_is_in_normal_form(spans in proptest::collection::vec(span(), 0..8)) {
            let text = RichText::new(spans.clone());
            prop_assert!(text.spans().iter().all(|span| !span.text.is_empty()));
            prop_assert!(text.spans().windows(2).all(|pair| pair[0].marks != pair[1].marks));
            let expected: String = spans.iter().map(|span| span.text.as_str()).collect();
            prop_assert_eq!(text.to_plain(), expected);
        }
    }
}
