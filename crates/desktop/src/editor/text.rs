use gpui_kit::SharedString;
use gpui_kit::base::text::editor_host::{
    CodeBlock, InlineNode, LinkMark, Paragraph, Span as SourceSpan, TextMark,
};
use remindr_core::domain::{CodeLanguage, Marks, RichText};

use super::caret::Marked;
use super::palette::Palette;

pub fn paragraph(text: &RichText, palette: &Palette, key: u64, struck: bool) -> Paragraph {
    let mut paragraph = Paragraph::default();
    if !text.is_empty() {
        let mut marks = Vec::with_capacity(text.spans().len());
        let mut start = 0;
        for span in text.spans() {
            let end = start + span.text.len();
            marks.push((start..end, text_mark(&span.marks, palette, struck)));
            start = end;
        }
        paragraph.push(InlineNode::new(text.to_plain()).marks(marks));
    }
    paragraph.with_leaf_key(key)
}

pub fn code_block(text: &RichText, language: &CodeLanguage, key: u64) -> CodeBlock {
    let language = (language != &CodeLanguage::plain_text())
        .then(|| SharedString::from(language.as_str().to_owned()));
    CodeBlock::new(
        SharedString::from(text.to_plain()),
        language,
        None::<SourceSpan>,
    )
    .with_leaf_key(key)
}

pub fn composed(text: &RichText, marked: &Marked) -> RichText {
    let marks = text.marks_at(marked.start());
    text.delete(marked.replaced.clone())
        .and_then(|rest| rest.insert(marked.start(), &marked.text, &marks))
        .unwrap_or_else(|_| text.clone())
}

fn text_mark(marks: &Marks, palette: &Palette, struck: bool) -> TextMark {
    let mut mark = TextMark {
        bold: marks.bold,
        italic: marks.italic,
        underline: marks.underline,
        strikethrough: marks.strikethrough || struck,
        code: marks.code,
        highlight: marks.color.and_then(|color| palette.highlight(color)),
        link: None,
    };
    if let Some(link) = &marks.link {
        mark.link = Some(LinkMark {
            url: SharedString::from(link.as_str().to_owned()),
            identifier: None,
            title: None,
        });
    }
    mark
}

#[cfg(test)]
mod tests {
    use remindr_core::domain::{BlockId, Link, Marks, RichText, Span, TextColor, Uuid};

    use super::{composed, paragraph};
    use crate::editor::caret::Marked;
    use crate::editor::palette::Palette;

    fn palette() -> Palette {
        Palette::uniform(gpui_kit::hsla(0., 0., 0.5, 1.))
    }

    fn span(text: &str, marks: Marks) -> Span {
        Span {
            text: text.to_owned(),
            marks,
        }
    }

    #[test]
    fn rich_text_becomes_one_inline_run_with_mark_ranges() {
        let text = RichText::new([
            span("plain ", Marks::default()),
            span(
                "bold",
                Marks {
                    bold: true,
                    ..Marks::default()
                },
            ),
            span(
                " site",
                Marks {
                    link: Link::try_from("https://example.com").ok(),
                    color: Some(TextColor::RedBackground),
                    ..Marks::default()
                },
            ),
        ]);
        let built = paragraph(&text, &palette(), 7, false);
        let runs = built.children();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text(), "plain bold site");
        let marks = runs[0].mark_runs();
        assert_eq!(marks.len(), 3);
        assert_eq!(marks[0].0, 0..6);
        assert!(marks[1].1.bold && marks[1].0 == (6..10));
        assert_eq!(
            marks[2].1.link.as_ref().map(|link| link.url.to_string()),
            Some("https://example.com".to_owned())
        );
        assert!(marks[2].1.highlight.is_some());
    }

    #[test]
    fn empty_text_is_an_empty_paragraph() {
        let built = paragraph(&RichText::default(), &palette(), 1, false);
        assert!(built.children().is_empty());
    }

    #[test]
    fn struck_paragraphs_strike_every_run() {
        let built = paragraph(&RichText::plain("done"), &palette(), 1, true);
        assert!(built.children()[0].mark_runs()[0].1.strikethrough);
    }

    #[test]
    fn composition_is_shown_without_touching_the_source_text() {
        let text = RichText::plain("hello");
        let marked = Marked {
            block: BlockId::from_uuid(Uuid::from_u128(1)),
            replaced: 5..5,
            text: "ni".to_owned(),
            cursor: 2..2,
        };
        assert_eq!(composed(&text, &marked).to_plain(), "helloni");
        assert_eq!(text.to_plain(), "hello");
        let over = Marked {
            replaced: 1..3,
            ..marked
        };
        assert_eq!(composed(&text, &over).to_plain(), "hnilo");
    }
}
