use std::ops::Range;
use std::time::Duration;

use gpui_kit::{Context, Task};
use remindr_core::application::TextPoint;
use remindr_core::domain::BlockId;
use unicode_segmentation::UnicodeSegmentation;

const BLINK_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: TextPoint,
    pub head: TextPoint,
}

impl Selection {
    pub fn collapsed(point: TextPoint) -> Self {
        Self {
            anchor: point,
            head: point,
        }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marked {
    pub block: BlockId,
    pub replaced: Range<usize>,
    pub text: String,
    pub cursor: Range<usize>,
}

impl Marked {
    pub fn start(&self) -> usize {
        self.replaced.start
    }

    pub fn display_range(&self) -> Range<usize> {
        self.replaced.start..self.replaced.start + self.text.len()
    }

    pub fn display_cursor(&self) -> Range<usize> {
        self.replaced.start + self.cursor.start..self.replaced.start + self.cursor.end
    }
}

pub struct Blink {
    active: bool,
    visible: bool,
    epoch: usize,
    task: Task<()>,
}

impl Blink {
    pub fn new() -> Self {
        Self {
            active: false,
            visible: false,
            epoch: 0,
            task: Task::ready(()),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.active = true;
        self.cycle(cx);
    }

    pub fn restart(&mut self, cx: &mut Context<Self>) {
        if self.active {
            self.cycle(cx);
        }
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.active = false;
        self.visible = false;
        self.epoch += 1;
        self.task = Task::ready(());
        cx.notify();
    }

    fn cycle(&mut self, cx: &mut Context<Self>) {
        self.visible = true;
        self.epoch += 1;
        self.schedule(self.epoch, cx);
        cx.notify();
    }

    fn schedule(&mut self, epoch: usize, cx: &mut Context<Self>) {
        self.task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BLINK_INTERVAL).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| this.toggle(epoch, cx));
            }
        });
    }

    fn toggle(&mut self, epoch: usize, cx: &mut Context<Self>) {
        if epoch != self.epoch {
            return;
        }
        self.visible = !self.visible;
        self.epoch += 1;
        self.schedule(self.epoch, cx);
        cx.notify();
    }
}

pub fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

pub fn grapheme_before(text: &str, offset: usize) -> Option<usize> {
    let offset = floor_boundary(text, offset);
    text.get(..offset)?
        .grapheme_indices(true)
        .next_back()
        .map(|(start, _)| start)
}

pub fn grapheme_after(text: &str, offset: usize) -> Option<usize> {
    let offset = floor_boundary(text, offset);
    text.get(offset..)?
        .graphemes(true)
        .next()
        .map(|grapheme| offset + grapheme.len())
}

pub fn word_before(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    let mut target = 0;
    let mut seen_word = false;
    for (start, segment) in text[..offset].split_word_bound_indices().rev() {
        let blank = segment.chars().all(char::is_whitespace);
        if !blank {
            seen_word = true;
        }
        if seen_word && blank {
            target = start + segment.len();
            break;
        }
        if seen_word && !is_word(segment) {
            target = start + segment.len();
            break;
        }
        target = start;
    }
    target
}

pub fn word_after(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    let mut target = text.len();
    let mut seen_word = false;
    for (start, segment) in text[offset..].split_word_bound_indices() {
        let blank = segment.chars().all(char::is_whitespace);
        if seen_word && (blank || !is_word(segment)) {
            target = offset + start;
            break;
        }
        if !blank {
            seen_word = true;
        }
    }
    target
}

pub fn word_range(text: &str, offset: usize) -> Range<usize> {
    let offset = floor_boundary(text, offset);
    let mut found = offset..offset;
    for (start, segment) in text.split_word_bound_indices() {
        let end = start + segment.len();
        if start <= offset && offset <= end {
            found = start..end;
            if offset < end {
                break;
            }
        }
    }
    found
}

fn is_word(segment: &str) -> bool {
    segment.chars().any(char::is_alphanumeric)
}

pub fn utf16_to_byte(text: &str, utf16: usize) -> usize {
    let mut count = 0;
    for (index, character) in text.char_indices() {
        if count >= utf16 {
            return index;
        }
        count += character.len_utf16();
    }
    text.len()
}

pub fn byte_to_utf16(text: &str, byte: usize) -> usize {
    text.get(..byte.min(text.len()))
        .map_or(0, |head| head.encode_utf16().count())
}

#[cfg(test)]
mod tests {
    use super::{
        Marked, byte_to_utf16, grapheme_after, grapheme_before, utf16_to_byte, word_after,
        word_before, word_range,
    };
    use remindr_core::domain::{BlockId, Uuid};

    #[test]
    fn graphemes_move_over_clusters() {
        let text = "a👍🏽é";
        assert_eq!(grapheme_after(text, 0), Some(1));
        assert_eq!(grapheme_after(text, 1), Some(1 + "👍🏽".len()));
        assert_eq!(grapheme_before(text, text.len()), Some(1 + "👍🏽".len()));
        assert_eq!(grapheme_before(text, 1), Some(0));
        assert_eq!(grapheme_before(text, 0), None);
        assert_eq!(grapheme_after(text, text.len()), None);
    }

    #[test]
    fn words_move_to_word_edges() {
        let text = "hello big  world";
        assert_eq!(word_after(text, 0), 5);
        assert_eq!(word_after(text, 5), 9);
        assert_eq!(word_after(text, 9), 16);
        assert_eq!(word_before(text, 16), 11);
        assert_eq!(word_before(text, 11), 6);
        assert_eq!(word_before(text, 6), 0);
        assert_eq!(word_range(text, 7), 6..9);
    }

    #[test]
    fn utf16_offsets_round_trip() {
        let text = "a😀b";
        assert_eq!(byte_to_utf16(text, 5), 3);
        assert_eq!(utf16_to_byte(text, 3), 5);
        assert_eq!(utf16_to_byte(text, 99), text.len());
    }

    #[test]
    fn marked_text_reports_display_ranges() {
        let marked = Marked {
            block: BlockId::from_uuid(Uuid::from_u128(1)),
            replaced: 2..4,
            text: "abcdef".to_owned(),
            cursor: 6..6,
        };
        assert_eq!(marked.display_range(), 2..8);
        assert_eq!(marked.display_cursor(), 8..8);
    }
}
