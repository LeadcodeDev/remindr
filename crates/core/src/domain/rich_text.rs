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
        let mut end = 0;
        let found = self.0.iter().find(|span| {
            end += span.text.len();
            end >= offset
        });
        let marks = found
            .or_else(|| self.0.last())
            .map(|span| span.marks.clone())
            .unwrap_or_default();
        Marks {
            link: None,
            ..marks
        }
    }

    pub fn split_at(&self, offset: usize) -> Result<(RichText, RichText), DomainError> {
        if offset > self.len() {
            return Err(DomainError::InvalidOffset(offset));
        }
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut start = 0;
        for span in &self.0 {
            let end = start + span.text.len();
            if end <= offset {
                left.push(span.clone());
            } else if start >= offset {
                right.push(span.clone());
            } else {
                let local = offset - start;
                if !span.text.is_char_boundary(local) {
                    return Err(DomainError::InvalidOffset(offset));
                }
                let (head, tail) = span.text.split_at(local);
                left.push(Span {
                    text: head.to_owned(),
                    marks: span.marks.clone(),
                });
                right.push(Span {
                    text: tail.to_owned(),
                    marks: span.marks.clone(),
                });
            }
            start = end;
        }
        Ok((RichText::new(left), RichText::new(right)))
    }

    #[must_use]
    pub fn concat(&self, other: &RichText) -> RichText {
        RichText::new(self.0.iter().chain(&other.0).cloned())
    }

    pub fn insert(
        &self,
        offset: usize,
        text: &str,
        marks: &Marks,
    ) -> Result<RichText, DomainError> {
        let (left, right) = self.split_at(offset)?;
        let inserted = RichText::new([Span {
            text: text.to_owned(),
            marks: marks.clone(),
        }]);
        Ok(left.concat(&inserted).concat(&right))
    }

    pub fn delete(&self, range: Range<usize>) -> Result<RichText, DomainError> {
        let (head, tail) = self.split_at(range.end)?;
        let (kept, _) = head.split_at(range.start)?;
        Ok(kept.concat(&tail))
    }

    pub fn apply_mark(
        &self,
        range: Range<usize>,
        change: &MarkChange,
    ) -> Result<RichText, DomainError> {
        let (head, tail) = self.split_at(range.end)?;
        let (before, middle) = head.split_at(range.start)?;
        let changed = RichText::new(middle.0.into_iter().map(|mut span| {
            change.set_on(&mut span.marks);
            span
        }));
        Ok(before.concat(&changed).concat(&tail))
    }

    #[must_use]
    pub fn has_mark(&self, range: Range<usize>, change: &MarkChange) -> bool {
        if range.start >= range.end || range.end > self.len() {
            return false;
        }
        let mut start = 0;
        self.0.iter().all(|span| {
            let end = start + span.text.len();
            let overlaps = start < range.end && end > range.start;
            start = end;
            !overlaps || change.is_set_on(&span.marks)
        })
    }
}

impl MarkChange {
    fn set_on(&self, marks: &mut Marks) {
        match self {
            Self::Bold(value) => marks.bold = *value,
            Self::Italic(value) => marks.italic = *value,
            Self::Underline(value) => marks.underline = *value,
            Self::Strikethrough(value) => marks.strikethrough = *value,
            Self::Code(value) => marks.code = *value,
            Self::Link(value) => marks.link.clone_from(value),
            Self::Color(value) => marks.color = *value,
        }
    }

    fn is_set_on(&self, marks: &Marks) -> bool {
        match self {
            Self::Bold(value) => marks.bold == *value,
            Self::Italic(value) => marks.italic == *value,
            Self::Underline(value) => marks.underline == *value,
            Self::Strikethrough(value) => marks.strikethrough == *value,
            Self::Code(value) => marks.code == *value,
            Self::Link(value) => marks.link == *value,
            Self::Color(value) => marks.color == *value,
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use std::ops::Range;

    use super::{Link, MarkChange, Marks, RichText, Span};
    use crate::domain::DomainError;

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

    fn boundaries(text: &RichText) -> Vec<usize> {
        let plain = text.to_plain();
        plain
            .char_indices()
            .map(|(index, _)| index)
            .chain([plain.len()])
            .collect()
    }

    fn bold() -> Marks {
        Marks {
            bold: true,
            ..Marks::default()
        }
    }

    fn two_part() -> RichText {
        RichText::new([
            Span {
                text: "ab".to_owned(),
                marks: Marks::default(),
            },
            Span {
                text: "cd".to_owned(),
                marks: bold(),
            },
        ])
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

    #[test]
    fn split_at_non_boundary_is_rejected() {
        assert_eq!(
            RichText::plain("é").split_at(1),
            Err(DomainError::InvalidOffset(1))
        );
    }

    #[test]
    fn split_past_the_end_is_rejected() {
        assert_eq!(
            RichText::plain("ab").split_at(3),
            Err(DomainError::InvalidOffset(3))
        );
    }

    #[test]
    fn split_inside_a_span_keeps_marks_on_both_sides() {
        let (left, right) = two_part().split_at(3).unwrap();
        assert_eq!(left.to_plain(), "abc");
        assert_eq!(right.to_plain(), "d");
        assert_eq!(left.spans()[1].marks, bold());
        assert_eq!(right.spans()[0].marks, bold());
    }

    #[test]
    fn split_of_empty_text_gives_two_empty_texts() {
        let (left, right) = RichText::default().split_at(0).unwrap();
        assert!(left.is_empty() && right.is_empty());
    }

    #[test]
    fn concat_merges_equal_marks_at_the_seam() {
        let joined = RichText::plain("ab").concat(&RichText::plain("cd"));
        assert_eq!(joined, RichText::plain("abcd"));
    }

    #[test]
    fn insert_places_text_with_the_given_marks() {
        let inserted = RichText::plain("ac").insert(1, "b", &bold()).unwrap();
        let texts: Vec<&str> = inserted.spans().iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["a", "b", "c"]);
        assert_eq!(inserted.spans()[1].marks, bold());
    }

    #[test]
    fn insert_with_plain_marks_extends_the_span() {
        let inserted = RichText::plain("ac")
            .insert(1, "b", &Marks::default())
            .unwrap();
        assert_eq!(inserted, RichText::plain("abc"));
    }

    #[test]
    fn insert_rejects_bad_offsets() {
        assert_eq!(
            RichText::plain("é").insert(1, "x", &Marks::default()),
            Err(DomainError::InvalidOffset(1))
        );
        assert_eq!(
            RichText::plain("a").insert(2, "x", &Marks::default()),
            Err(DomainError::InvalidOffset(2))
        );
    }

    #[test]
    fn delete_removes_the_range_across_spans() {
        let deleted = two_part().delete(1..3).unwrap();
        assert_eq!(deleted.to_plain(), "ad");
        assert_eq!(deleted.spans().len(), 2);
    }

    #[test]
    fn delete_rejects_reversed_out_of_range_and_non_boundary_ranges() {
        let text = RichText::plain("aé");
        let reversed = Range { start: 2, end: 1 };
        assert_eq!(text.delete(reversed), Err(DomainError::InvalidOffset(2)));
        assert_eq!(text.delete(0..9), Err(DomainError::InvalidOffset(9)));
        assert_eq!(text.delete(0..2), Err(DomainError::InvalidOffset(2)));
    }

    #[test]
    fn delete_of_an_empty_range_changes_nothing() {
        assert_eq!(two_part().delete(2..2).unwrap(), two_part());
    }

    #[test]
    fn apply_bold_on_world_gives_two_spans() {
        let marked = RichText::plain("Hello world")
            .apply_mark(6..11, &MarkChange::Bold(true))
            .unwrap();
        assert_eq!(
            marked.spans(),
            [
                Span {
                    text: "Hello ".to_owned(),
                    marks: Marks::default()
                },
                Span {
                    text: "world".to_owned(),
                    marks: bold()
                },
            ]
        );
    }

    #[test]
    fn apply_bold_false_removes_bold() {
        let unmarked = two_part()
            .apply_mark(0..4, &MarkChange::Bold(false))
            .unwrap();
        assert_eq!(unmarked, RichText::plain("abcd"));
    }

    #[test]
    fn apply_link_and_color_set_their_values() {
        let link = Link::try_from("https://x.dev").unwrap();
        let linked = RichText::plain("ab")
            .apply_mark(0..1, &MarkChange::Link(Some(link.clone())))
            .unwrap();
        assert_eq!(linked.spans()[0].marks.link, Some(link));
        assert_eq!(linked.spans()[1].marks.link, None);
    }

    #[test]
    fn apply_mark_rejects_bad_ranges() {
        let text = RichText::plain("aé");
        assert_eq!(
            text.apply_mark(0..9, &MarkChange::Italic(true)),
            Err(DomainError::InvalidOffset(9))
        );
        assert_eq!(
            text.apply_mark(2..3, &MarkChange::Italic(true)),
            Err(DomainError::InvalidOffset(2))
        );
    }

    #[test]
    fn has_mark_is_false_on_partial_coverage() {
        let text = two_part();
        assert!(text.has_mark(2..4, &MarkChange::Bold(true)));
        assert!(!text.has_mark(1..4, &MarkChange::Bold(true)));
        assert!(text.has_mark(0..2, &MarkChange::Bold(false)));
        assert!(!text.has_mark(0..4, &MarkChange::Bold(false)));
    }

    #[test]
    fn has_mark_is_false_on_empty_or_out_of_range() {
        let text = two_part();
        assert!(!text.has_mark(2..2, &MarkChange::Bold(true)));
        assert!(!text.has_mark(2..9, &MarkChange::Bold(true)));
        assert!(!RichText::default().has_mark(0..0, &MarkChange::Bold(false)));
    }

    #[test]
    fn marks_at_drops_link() {
        let link = Link::try_from("https://x.dev").unwrap();
        let text = RichText::plain("ab")
            .apply_mark(0..2, &MarkChange::Link(Some(link)))
            .unwrap()
            .apply_mark(0..2, &MarkChange::Bold(true))
            .unwrap();
        assert_eq!(text.marks_at(1), bold());
        assert_eq!(text.marks_at(2), bold());
    }

    #[test]
    fn marks_at_uses_the_char_before_the_offset() {
        let text = two_part();
        assert_eq!(text.marks_at(0), Marks::default());
        assert_eq!(text.marks_at(2), Marks::default());
        assert_eq!(text.marks_at(3), bold());
        assert_eq!(text.marks_at(4), bold());
        assert_eq!(RichText::default().marks_at(0), Marks::default());
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

        #[test]
        fn split_then_concat_is_identity(
            spans in proptest::collection::vec(span(), 0..8),
            pick in any::<prop::sample::Index>(),
        ) {
            let text = RichText::new(spans);
            let points = boundaries(&text);
            let offset = points[pick.index(points.len())];
            let (left, right) = text.split_at(offset).unwrap();
            prop_assert_eq!(left.len(), offset);
            prop_assert_eq!(left.concat(&right), text);
        }

        #[test]
        fn insert_then_delete_is_identity(
            spans in proptest::collection::vec(span(), 0..8),
            pick in any::<prop::sample::Index>(),
            inserted in "[a-zé]{0,3}",
            bold in any::<bool>(),
        ) {
            let text = RichText::new(spans);
            let points = boundaries(&text);
            let offset = points[pick.index(points.len())];
            let marks = Marks { bold, ..Marks::default() };
            let grown = text.insert(offset, &inserted, &marks).unwrap();
            prop_assert_eq!(grown.len(), text.len() + inserted.len());
            let restored = grown.delete(offset..offset + inserted.len()).unwrap();
            prop_assert_eq!(restored, text);
        }

        #[test]
        fn apply_mark_keeps_text_and_normal_form(
            spans in proptest::collection::vec(span(), 0..8),
            first in any::<prop::sample::Index>(),
            second in any::<prop::sample::Index>(),
            value in any::<bool>(),
        ) {
            let text = RichText::new(spans);
            let points = boundaries(&text);
            let a = points[first.index(points.len())];
            let b = points[second.index(points.len())];
            let (start, end) = (a.min(b), a.max(b));
            let change = MarkChange::Bold(value);
            let marked = text.apply_mark(start..end, &change).unwrap();
            prop_assert_eq!(marked.to_plain(), text.to_plain());
            prop_assert!(marked.spans().windows(2).all(|pair| pair[0].marks != pair[1].marks));
            prop_assert!(start == end || marked.has_mark(start..end, &change));
            let again = marked.apply_mark(start..end, &change).unwrap();
            prop_assert_eq!(again, marked);
        }
    }
}
