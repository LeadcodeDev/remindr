use gpui_kit::{ClipboardItem, Context, Window};
use remindr_core::application::{Command, TextPoint};
use remindr_core::domain::{BlockKind, HeadingLevel, Workspace};

use super::rows::{Row, extent, position};
use super::{BlockEditor, is_atomic_row};

pub fn selection_markdown(
    workspace: &Workspace,
    rows: &[Row],
    start: TextPoint,
    end: TextPoint,
) -> String {
    let (Some(first), Some(last)) = (position(rows, start.block), position(rows, end.block)) else {
        return String::new();
    };
    if first > last {
        return String::new();
    }
    if first == last {
        return workspace
            .block(start.block)
            .map(|block| block.text.to_plain())
            .and_then(|plain| plain.get(start.offset..end.offset).map(str::to_owned))
            .unwrap_or_default();
    }
    let selected = rows.get(first..=last).unwrap_or_default();
    let base = selected.iter().map(|row| row.depth).min().unwrap_or(0);
    let mut lines = Vec::with_capacity(selected.len());
    for (index, row) in selected.iter().enumerate() {
        let Some(block) = workspace.block(row.block) else {
            continue;
        };
        let plain = block.text.to_plain();
        let from = if index == 0 { start.offset } else { 0 };
        let to = if index + 1 == selected.len() {
            end.offset
        } else {
            plain.len()
        };
        let slice = plain.get(from..to).unwrap_or_default();
        if let Some(line) = markdown_line(&block.kind, row, slice) {
            lines.push(format!("{}{line}", "  ".repeat(row.depth - base)));
        }
    }
    lines.join("\n")
}

fn markdown_line(kind: &BlockKind, row: &Row, slice: &str) -> Option<String> {
    let prefixed = |prefix: &str| Some(format!("{prefix}{slice}"));
    match kind {
        BlockKind::Text | BlockKind::Page { .. } => Some(slice.to_owned()),
        BlockKind::Heading(HeadingLevel::One) => prefixed("# "),
        BlockKind::Heading(HeadingLevel::Two) => prefixed("## "),
        BlockKind::Heading(HeadingLevel::Three) => prefixed("### "),
        BlockKind::BulletedListItem | BlockKind::Toggle => prefixed("- "),
        BlockKind::NumberedListItem => prefixed(&format!("{}. ", row.number.unwrap_or(1))),
        BlockKind::ToDo { checked: false } => prefixed("- [ ] "),
        BlockKind::ToDo { checked: true } => prefixed("- [x] "),
        BlockKind::Quote | BlockKind::Callout { .. } => prefixed("> "),
        BlockKind::Code { language } => {
            let name = if language.as_str() == "plain text" {
                ""
            } else {
                language.as_str()
            };
            Some(format!("```{name}\n{slice}\n```"))
        }
        BlockKind::Divider => Some("---".to_owned()),
        BlockKind::DatabaseView { .. } => None,
    }
}

impl BlockEditor {
    fn selected_markdown(&self, cx: &Context<Self>) -> Option<String> {
        let selection = self
            .selection
            .filter(|selection| !selection.is_collapsed())?;
        let rows = self.visible_rows(cx);
        let span = extent(&rows, selection.anchor, selection.head)?;
        let markdown =
            selection_markdown(self.model.read(cx).workspace(), &rows, span.start, span.end);
        (!markdown.is_empty()).then_some(markdown)
    }

    pub(super) fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(markdown) = self.selected_markdown(cx) {
            cx.write_to_clipboard(ClipboardItem::new_string(markdown));
        }
    }

    pub(super) fn cut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_marked(window, cx);
        self.copy(cx);
        if let Some(selection) = self.selection.filter(|selection| !selection.is_collapsed()) {
            self.delete_selection(selection, window, cx);
        }
    }

    pub(super) fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_marked(window, cx);
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        self.paste_text(&text, window, cx);
    }

    pub(super) fn paste_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }
        let Some(at) = self.insertion_point(window, cx) else {
            return;
        };
        let atomic = self
            .kind_of(at.block, cx)
            .is_none_or(|kind| is_atomic_row(&kind));
        if atomic {
            return;
        }
        let command = Command::PasteText {
            at,
            text: text.to_owned(),
        };
        self.dispatch(command, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use remindr_core::application::TextPoint;
    use remindr_core::domain::{BlockKind, CodeLanguage, HeadingLevel};

    use super::selection_markdown;
    use crate::editor::rows::{Expansion, rows};
    use crate::editor::testing::Doc;

    #[test]
    fn selection_copies_as_markdown() {
        let mut doc = Doc::new();
        let title = doc.child(BlockKind::Heading(HeadingLevel::One), "Title");
        let item = doc.child(BlockKind::BulletedListItem, "item");
        doc.under(item, BlockKind::ToDo { checked: true }, "done");
        doc.child(BlockKind::NumberedListItem, "first");
        doc.child(BlockKind::Quote, "wise");
        doc.child(
            BlockKind::Code {
                language: CodeLanguage::new("rust"),
            },
            "let x = 1;",
        );
        doc.child(BlockKind::Divider, "");
        let tail = doc.child(BlockKind::Text, "tail");
        let workspace = doc.workspace();
        let found = rows(&workspace, doc.page(), Expansion::All);
        let markdown = selection_markdown(
            &workspace,
            &found,
            TextPoint {
                block: title,
                offset: 0,
            },
            TextPoint {
                block: tail,
                offset: 2,
            },
        );
        assert_eq!(
            markdown,
            "# Title\n- item\n  - [x] done\n1. first\n> wise\n```rust\nlet x = 1;\n```\n---\nta"
        );
    }

    #[test]
    fn a_selection_inside_one_block_copies_plain_text() {
        let mut doc = Doc::new();
        let title = doc.child(BlockKind::Heading(HeadingLevel::Two), "Hello world");
        let workspace = doc.workspace();
        let found = rows(&workspace, doc.page(), Expansion::All);
        let copied = selection_markdown(
            &workspace,
            &found,
            TextPoint {
                block: title,
                offset: 6,
            },
            TextPoint {
                block: title,
                offset: 11,
            },
        );
        assert_eq!(copied, "world");
    }
}
