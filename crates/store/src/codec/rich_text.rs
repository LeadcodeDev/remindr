use std::borrow::Cow;

use remindr_core::domain::{Link, Marks, RichText, Span, TextColor};
use remindr_core::ports::StoreError;
use serde::{Deserialize, Serialize};

use super::{Invalid, Named, corrupt, decode, named, to_json};

const COLUMN: &str = "block.text";

named!(TextColor {
    TextColor::Gray => "gray",
    TextColor::Brown => "brown",
    TextColor::Orange => "orange",
    TextColor::Yellow => "yellow",
    TextColor::Green => "green",
    TextColor::Blue => "blue",
    TextColor::Purple => "purple",
    TextColor::Pink => "pink",
    TextColor::Red => "red",
    TextColor::GrayBackground => "gray_background",
    TextColor::BrownBackground => "brown_background",
    TextColor::OrangeBackground => "orange_background",
    TextColor::YellowBackground => "yellow_background",
    TextColor::GreenBackground => "green_background",
    TextColor::BlueBackground => "blue_background",
    TextColor::PurpleBackground => "purple_background",
    TextColor::PinkBackground => "pink_background",
    TextColor::RedBackground => "red_background",
});

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Annotation<'a> {
    Flag((Cow<'a, str>,)),
    Pair(Cow<'a, str>, Cow<'a, str>),
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Segment<'a> {
    Plain((Cow<'a, str>,)),
    Marked(Cow<'a, str>, Vec<Annotation<'a>>),
}

pub(crate) fn encode_text(text: &RichText) -> Result<String, StoreError> {
    let segments: Vec<Segment<'_>> = text.spans().iter().map(segment).collect();
    to_json(&segments)
}

fn segment(span: &Span) -> Segment<'_> {
    let text = Cow::Borrowed(span.text.as_str());
    let annotations = annotations(&span.marks);
    if annotations.is_empty() {
        Segment::Plain((text,))
    } else {
        Segment::Marked(text, annotations)
    }
}

fn annotations(marks: &Marks) -> Vec<Annotation<'_>> {
    let flags = [
        (marks.bold, "b"),
        (marks.italic, "i"),
        (marks.underline, "_"),
        (marks.strikethrough, "s"),
        (marks.code, "c"),
    ];
    let mut annotations: Vec<Annotation<'_>> = flags
        .into_iter()
        .filter(|(enabled, _)| *enabled)
        .map(|(_, code)| Annotation::Flag((Cow::Borrowed(code),)))
        .collect();
    if let Some(link) = &marks.link {
        annotations.push(Annotation::Pair(
            Cow::Borrowed("a"),
            Cow::Borrowed(link.as_str()),
        ));
    }
    if let Some(color) = marks.color {
        annotations.push(Annotation::Pair(
            Cow::Borrowed("h"),
            Cow::Borrowed(color.name()),
        ));
    }
    annotations
}

pub(crate) fn decode_text(raw: &str) -> Result<RichText, StoreError> {
    let segments: Vec<Segment<'_>> = decode(COLUMN, raw)?;
    let spans = segments
        .into_iter()
        .map(span)
        .collect::<Result<Vec<_>, Invalid>>()
        .map_err(corrupt(COLUMN))?;
    Ok(RichText::new(spans))
}

fn span(segment: Segment<'_>) -> Result<Span, Invalid> {
    match segment {
        Segment::Plain((text,)) => Ok(Span {
            text: text.into_owned(),
            marks: Marks::default(),
        }),
        Segment::Marked(text, annotations) => Ok(Span {
            text: text.into_owned(),
            marks: marks(&annotations)?,
        }),
    }
}

fn marks(annotations: &[Annotation<'_>]) -> Result<Marks, Invalid> {
    let mut marks = Marks::default();
    for annotation in annotations {
        apply(&mut marks, annotation)?;
    }
    Ok(marks)
}

fn apply(marks: &mut Marks, annotation: &Annotation<'_>) -> Result<(), Invalid> {
    match annotation {
        Annotation::Flag((code,)) => match code.as_ref() {
            "b" => marks.bold = true,
            "i" => marks.italic = true,
            "_" => marks.underline = true,
            "s" => marks.strikethrough = true,
            "c" => marks.code = true,
            other => return Err(unknown_annotation(other)),
        },
        Annotation::Pair(code, argument) => match code.as_ref() {
            "a" => marks.link = Some(Link::try_from(argument.as_ref())?),
            "h" => marks.color = Some(named(argument, "color")?),
            other => return Err(unknown_annotation(other)),
        },
    }
    Ok(())
}

fn unknown_annotation(code: &str) -> Invalid {
    Invalid::new(format!("unknown annotation `{code}`"))
}

#[cfg(test)]
mod tests {
    use remindr_core::domain::{Link, Marks, RichText, Span, TextColor};
    use remindr_core::ports::StoreError;

    use super::{decode_text, encode_text};

    fn span(text: &str, marks: Marks) -> Span {
        Span {
            text: text.to_owned(),
            marks,
        }
    }

    fn everything() -> Marks {
        Marks {
            bold: true,
            italic: true,
            underline: true,
            strikethrough: true,
            code: true,
            link: Some(Link::try_from("https://x.dev").unwrap()),
            color: Some(TextColor::BlueBackground),
        }
    }

    #[test]
    fn rich_text_encodes_notion_segments() {
        let marks = Marks {
            bold: true,
            link: Some(Link::try_from("https://x.dev").unwrap()),
            ..Marks::default()
        };
        let text = RichText::new([span("Hello ", Marks::default()), span("world", marks)]);
        assert_eq!(
            encode_text(&text).unwrap(),
            r#"[["Hello "],["world",[["b"],["a","https://x.dev"]]]]"#
        );
    }

    #[test]
    fn empty_text_encodes_to_an_empty_array() {
        assert_eq!(encode_text(&RichText::default()).unwrap(), "[]");
        assert_eq!(decode_text("[]").unwrap(), RichText::default());
    }

    #[test]
    fn annotations_use_the_documented_order_and_codes() {
        let text = RichText::new([span("x", everything())]);
        assert_eq!(
            encode_text(&text).unwrap(),
            r#"[["x",[["b"],["i"],["_"],["s"],["c"],["a","https://x.dev"],["h","blue_background"]]]]"#
        );
    }

    #[test]
    fn rich_text_round_trips() {
        let text = RichText::new([
            span("plain ", Marks::default()),
            span("loaded é\"\\", everything()),
            span(
                " tail",
                Marks {
                    color: Some(TextColor::Red),
                    ..Marks::default()
                },
            ),
        ]);
        assert_eq!(decode_text(&encode_text(&text).unwrap()).unwrap(), text);
    }

    #[test]
    fn decoding_accepts_any_annotation_order() {
        let text = decode_text(r#"[["x",[["h","gray"],["b"]]]]"#).unwrap();
        let expected = Marks {
            bold: true,
            color: Some(TextColor::Gray),
            ..Marks::default()
        };
        assert_eq!(text, RichText::new([span("x", expected)]));
    }

    #[test]
    fn bad_json_and_unknown_codes_are_corrupt() {
        for raw in [
            "not json",
            r#"[["x",[["zz"]]]]"#,
            r#"[["x",[["h","mauve"]]]]"#,
            r#"[["x",[["a","a b"]]]]"#,
            r#"[["x",[["b","extra"]]]]"#,
            r#"{"a":1}"#,
        ] {
            match decode_text(raw) {
                Err(StoreError::Corrupt(message)) => {
                    assert!(message.starts_with("block.text: "), "{message}");
                }
                other => panic!("{raw}: {other:?}"),
            }
        }
    }
}
