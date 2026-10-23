use gpui_kit::{Context, Window};
use remindr_core::application::{Command, TextPoint};
use remindr_core::domain::{BlockKind, CodeLanguage, HeadingLevel};

use super::BlockEditor;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trigger {
    Kind(BlockKind),
    Divider,
}

pub fn trigger(before: &str) -> Option<Trigger> {
    let kind = match before {
        "# " => BlockKind::Heading(HeadingLevel::One),
        "## " => BlockKind::Heading(HeadingLevel::Two),
        "### " => BlockKind::Heading(HeadingLevel::Three),
        "- " | "* " => BlockKind::BulletedListItem,
        "1. " => BlockKind::NumberedListItem,
        "[] " => BlockKind::ToDo { checked: false },
        "> " => BlockKind::Toggle,
        "\" " => BlockKind::Quote,
        "```" => BlockKind::Code {
            language: CodeLanguage::plain_text(),
        },
        "---" => return Some(Trigger::Divider),
        _ => return None,
    };
    Some(Trigger::Kind(kind))
}

impl BlockEditor {
    pub(super) fn after_typed(
        &mut self,
        at: TextPoint,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let kind = self.kind_of(at.block, cx);
        if text == "/" && !matches!(kind, Some(BlockKind::Code { .. }) | None) {
            self.open_slash(at, cx);
            return;
        }
        if kind == Some(BlockKind::Text) {
            self.apply_shortcut(window, cx);
        }
    }

    fn apply_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.selection.filter(|selection| selection.is_collapsed()) else {
            return;
        };
        let head = selection.head;
        let Some(plain) = self.plain_of(head.block, cx) else {
            return;
        };
        let Some(trigger) = plain.get(..head.offset).and_then(trigger) else {
            return;
        };
        let block = head.block;
        match trigger {
            Trigger::Kind(kind) => {
                let start = TextPoint { block, offset: 0 };
                let strip = Command::DeleteRange { start, end: head };
                if self.dispatch(strip, window, cx).is_some() {
                    self.dispatch(Command::ChangeKind { block, kind }, window, cx);
                }
            }
            Trigger::Divider => {
                let divider = Command::ChangeKind {
                    block,
                    kind: BlockKind::Divider,
                };
                if self.dispatch(divider, window, cx).is_some() {
                    let next = Command::InsertBlock {
                        after: block,
                        kind: BlockKind::Text,
                    };
                    self.dispatch(next, window, cx);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::TestAppContext;
    use remindr_core::application::TextPoint;
    use remindr_core::domain::{BlockId, BlockKind, CodeLanguage, HeadingLevel};

    use super::{Trigger, trigger};
    use crate::editor::testing::{Doc, Fixture};

    #[test]
    fn the_trigger_must_be_the_whole_text_before_the_caret() {
        assert_eq!(
            trigger("# "),
            Some(Trigger::Kind(BlockKind::Heading(HeadingLevel::One)))
        );
        assert_eq!(trigger("---"), Some(Trigger::Divider));
        assert_eq!(trigger(" # "), None);
        assert_eq!(trigger("#"), None);
        assert_eq!(trigger("a- "), None);
        assert_eq!(trigger("1."), None);
        assert_eq!(trigger("2. "), None);
        assert_eq!(trigger("####"), None);
    }

    fn at(block: BlockId, offset: usize) -> TextPoint {
        TextPoint { block, offset }
    }

    fn typed(
        cx: &mut TestAppContext,
        kind: BlockKind,
        text: &str,
        typing: &str,
    ) -> (Fixture, BlockId) {
        let mut doc = Doc::new();
        let block = doc.child(kind, text);
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 0));
        fixture.input(cx, typing);
        (fixture, block)
    }

    fn shortcut(cx: &mut TestAppContext, typing: &str) -> (Fixture, BlockId) {
        typed(cx, BlockKind::Text, "", typing)
    }

    #[gpui_kit::test]
    fn hash_space_makes_a_heading_1(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "# ");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::Heading(HeadingLevel::One))
        );
        assert_eq!(fixture.text(cx, block), "");
        assert_eq!(fixture.caret(cx), Some(at(block, 0)));
    }

    #[gpui_kit::test]
    fn two_hashes_make_a_heading_2(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "## ");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::Heading(HeadingLevel::Two))
        );
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn three_hashes_make_a_heading_3(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "### ");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::Heading(HeadingLevel::Three))
        );
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn dash_space_makes_a_bulleted_item(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "- ");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::BulletedListItem));
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn star_space_makes_a_bulleted_item(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "* ");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::BulletedListItem));
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn one_dot_space_makes_a_numbered_item(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "1. ");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::NumberedListItem));
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn brackets_space_make_a_todo(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "[] ");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::ToDo { checked: false })
        );
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn greater_than_space_makes_a_toggle(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "> ");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Toggle));
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn quote_space_makes_a_quote(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "\" ");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Quote));
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn three_backticks_make_a_code_block(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "```");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::Code {
                language: CodeLanguage::plain_text()
            })
        );
        assert_eq!(fixture.text(cx, block), "");
        assert_eq!(fixture.caret(cx), Some(at(block, 0)));
    }

    #[gpui_kit::test]
    fn three_dashes_make_a_divider_and_a_text_block_to_continue(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "---");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Divider));
        let children = fixture.children_ids(cx, fixture.page);
        assert_eq!(children.len(), 2);
        assert_eq!(children[0], block);
        assert_eq!(fixture.kind(cx, children[1]), Some(BlockKind::Text));
        assert_eq!(fixture.caret(cx), Some(at(children[1], 0)));
        fixture.input(cx, "next");
        assert_eq!(fixture.text(cx, children[1]), "next");
    }

    #[gpui_kit::test]
    fn two_dashes_and_a_space_stay_text(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "-- ");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Text));
        assert_eq!(fixture.text(cx, block), "-- ");
    }

    #[gpui_kit::test]
    fn the_rest_of_the_block_stays_after_the_prefix(cx: &mut TestAppContext) {
        let (fixture, block) = typed(cx, BlockKind::Text, "rest", "## ");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::Heading(HeadingLevel::Two))
        );
        assert_eq!(fixture.text(cx, block), "rest");
        assert_eq!(fixture.caret(cx), Some(at(block, 0)));
    }

    #[gpui_kit::test]
    fn a_prefix_in_the_middle_of_text_is_just_text(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "a");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 1));
        fixture.input(cx, "# ");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Text));
        assert_eq!(fixture.text(cx, block), "a# ");
    }

    #[gpui_kit::test]
    fn shortcuts_only_apply_to_text_blocks(cx: &mut TestAppContext) {
        let (fixture, block) = typed(cx, BlockKind::Heading(HeadingLevel::One), "", "- ");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::Heading(HeadingLevel::One))
        );
        assert_eq!(fixture.text(cx, block), "- ");
        let (fixture, block) = typed(
            cx,
            BlockKind::Code {
                language: CodeLanguage::plain_text(),
            },
            "",
            "```",
        );
        assert_eq!(fixture.text(cx, block), "```");
    }

    #[gpui_kit::test]
    fn undo_after_a_shortcut_restores_the_kind_then_the_prefix(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "# ");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::Heading(HeadingLevel::One))
        );
        fixture.press(cx, "secondary-z");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Text));
        assert_eq!(fixture.text(cx, block), "");
        fixture.press(cx, "secondary-z");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Text));
        assert_eq!(fixture.text(cx, block), "# ");
        fixture.press(cx, "secondary-z");
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn a_shortcut_does_not_open_the_slash_menu_or_misfire_on_slash(cx: &mut TestAppContext) {
        let (fixture, block) = shortcut(cx, "/");
        assert!(fixture.shown(cx, "slash-menu"));
        assert_eq!(fixture.text(cx, block), "/");
    }

    #[gpui_kit::test]
    fn a_slash_in_a_code_block_is_plain_text(cx: &mut TestAppContext) {
        let (fixture, block) = typed(
            cx,
            BlockKind::Code {
                language: CodeLanguage::plain_text(),
            },
            "",
            "/",
        );
        assert!(!fixture.shown(cx, "slash-menu"));
        assert_eq!(fixture.text(cx, block), "/");
    }
}
