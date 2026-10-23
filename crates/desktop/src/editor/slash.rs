use gpui_kit::{App, Context, ScrollHandle, SharedString, Window};
use remindr_core::application::{Command, TextPoint};
use remindr_core::domain::{
    BlockId, BlockKind, CodeLanguage, DatabaseId, Emoji, HeadingLevel, Parent,
};
use unicode_segmentation::UnicodeSegmentation;

use super::BlockEditor;
use super::overlay::{MenuRow, Overlay};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Text,
    Heading(HeadingLevel),
    Bulleted,
    Numbered,
    ToDo,
    Toggle,
    Quote,
    Callout,
    Code,
    Divider,
    Page,
    Database,
    DatabaseView,
}

pub struct Item {
    pub label: &'static str,
    pub aliases: &'static [&'static str],
    pub choice: Choice,
}

const NO_DATABASE: &str = "No database yet";
const CALLOUT_ICON: &str = "\u{1F4A1}";

pub const ITEMS: [Item; 15] = [
    Item {
        label: "Text",
        aliases: &["text", "paragraph"],
        choice: Choice::Text,
    },
    Item {
        label: "Heading 1",
        aliases: &["h1"],
        choice: Choice::Heading(HeadingLevel::One),
    },
    Item {
        label: "Heading 2",
        aliases: &["h2"],
        choice: Choice::Heading(HeadingLevel::Two),
    },
    Item {
        label: "Heading 3",
        aliases: &["h3"],
        choice: Choice::Heading(HeadingLevel::Three),
    },
    Item {
        label: "Bulleted list",
        aliases: &["bullet", "ul"],
        choice: Choice::Bulleted,
    },
    Item {
        label: "Numbered list",
        aliases: &["number", "ol"],
        choice: Choice::Numbered,
    },
    Item {
        label: "To-do",
        aliases: &["todo", "check"],
        choice: Choice::ToDo,
    },
    Item {
        label: "Toggle",
        aliases: &[],
        choice: Choice::Toggle,
    },
    Item {
        label: "Quote",
        aliases: &[],
        choice: Choice::Quote,
    },
    Item {
        label: "Callout",
        aliases: &[],
        choice: Choice::Callout,
    },
    Item {
        label: "Code",
        aliases: &[],
        choice: Choice::Code,
    },
    Item {
        label: "Divider",
        aliases: &["hr"],
        choice: Choice::Divider,
    },
    Item {
        label: "Page",
        aliases: &[],
        choice: Choice::Page,
    },
    Item {
        label: "Database",
        aliases: &["table", "board"],
        choice: Choice::Database,
    },
    Item {
        label: "Database view",
        aliases: &["linked"],
        choice: Choice::DatabaseView,
    },
];

pub fn filter(query: &str) -> Vec<&'static Item> {
    let needle = query.to_lowercase();
    let mut leading = Vec::new();
    let mut inside = Vec::new();
    for item in &ITEMS {
        let names = || {
            std::iter::once(item.label)
                .chain(item.aliases.iter().copied())
                .map(str::to_lowercase)
        };
        if names().any(|name| name.starts_with(&needle)) {
            leading.push(item);
        } else if names().any(|name| name.contains(&needle)) {
            inside.push(item);
        }
    }
    leading.extend(inside);
    leading
}

fn kind_of(choice: Choice) -> Option<BlockKind> {
    Some(match choice {
        Choice::Text => BlockKind::Text,
        Choice::Heading(level) => BlockKind::Heading(level),
        Choice::Bulleted => BlockKind::BulletedListItem,
        Choice::Numbered => BlockKind::NumberedListItem,
        Choice::ToDo => BlockKind::ToDo { checked: false },
        Choice::Toggle => BlockKind::Toggle,
        Choice::Quote => BlockKind::Quote,
        Choice::Callout => BlockKind::Callout {
            icon: Emoji::try_from(CALLOUT_ICON).ok()?,
        },
        Choice::Code => BlockKind::Code {
            language: CodeLanguage::plain_text(),
        },
        Choice::Divider => BlockKind::Divider,
        Choice::Page | Choice::Database | Choice::DatabaseView => return None,
    })
}

pub struct Slash {
    pub block: BlockId,
    pub start: usize,
    pub query: String,
    pub selected: usize,
    pub scroll: ScrollHandle,
}

impl Slash {
    fn new(block: BlockId, start: usize) -> Self {
        Self {
            block,
            start,
            query: String::new(),
            selected: 0,
            scroll: ScrollHandle::new(),
        }
    }
}

pub struct Picker {
    pub block: BlockId,
    pub start: usize,
    pub query: String,
    pub selected: usize,
    pub scroll: ScrollHandle,
}

impl Picker {
    fn new(block: BlockId, start: usize) -> Self {
        Self {
            block,
            start,
            query: String::new(),
            selected: 0,
            scroll: ScrollHandle::new(),
        }
    }
}

pub struct Entry {
    pub item: &'static Item,
    pub enabled: bool,
}

pub struct DatabaseEntry {
    pub id: DatabaseId,
    pub name: String,
    pub icon: Option<String>,
}

fn step_index(current: usize, count: usize, down: bool, enabled: impl Fn(usize) -> bool) -> usize {
    let mut index = current;
    for _ in 0..count {
        index = if down {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        };
        if enabled(index) {
            return index;
        }
    }
    current
}

impl BlockEditor {
    pub(super) fn open_slash(&mut self, at: TextPoint, cx: &mut Context<Self>) {
        self.overlay = Some(Overlay::Slash(Slash::new(at.block, at.offset)));
        cx.notify();
    }

    fn slash_query(&self, cx: &App) -> Option<String> {
        let Some(Overlay::Slash(slash)) = &self.overlay else {
            return None;
        };
        let head = self
            .selection
            .filter(|selection| selection.is_collapsed())?
            .head;
        if head.block != slash.block || head.offset <= slash.start {
            return None;
        }
        let plain = self.plain_of(slash.block, cx)?;
        if plain.get(slash.start..slash.start + 1) != Some("/") {
            return None;
        }
        let query = plain.get(slash.start + 1..head.offset)?;
        if query.starts_with(' ') || query.contains('\n') {
            return None;
        }
        Some(query.to_owned())
    }

    pub(super) fn sync_slash(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.overlay, Some(Overlay::Slash(_))) {
            return;
        }
        match self.slash_query(cx) {
            None => {
                self.overlay = None;
                cx.notify();
            }
            Some(query) => {
                if let Some(Overlay::Slash(slash)) = &mut self.overlay
                    && slash.query != query
                {
                    slash.query = query;
                    slash.selected = 0;
                    slash.scroll.scroll_to_item(0);
                }
            }
        }
    }

    fn has_database(&self, cx: &App) -> bool {
        self.model.read(cx).workspace().databases().next().is_some()
    }

    pub(super) fn slash_entries(&self, slash: &Slash, cx: &App) -> Vec<Entry> {
        let linked = self.has_database(cx);
        filter(&slash.query)
            .into_iter()
            .map(|item| Entry {
                item,
                enabled: linked || item.choice != Choice::DatabaseView,
            })
            .collect()
    }

    pub(super) fn slash_rows(&self, slash: &Slash, cx: &App) -> Vec<MenuRow> {
        self.slash_entries(slash, cx)
            .into_iter()
            .map(|entry| MenuRow {
                label: SharedString::from(entry.item.label),
                detail: (!entry.enabled).then(|| SharedString::from(NO_DATABASE)),
                icon: None,
                enabled: entry.enabled,
            })
            .collect()
    }

    pub(super) fn slash_step(&mut self, down: bool, cx: &mut Context<Self>) {
        let Some(Overlay::Slash(slash)) = &self.overlay else {
            return;
        };
        let entries = self.slash_entries(slash, cx);
        let count = entries.len();
        if count == 0 {
            return;
        }
        let next = step_index(slash.selected, count, down, |index| entries[index].enabled);
        if let Some(Overlay::Slash(slash)) = &mut self.overlay {
            slash.selected = next;
            slash.scroll.scroll_to_item(next);
        }
        cx.notify();
    }

    pub(super) fn slash_pick(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Overlay::Slash(slash)) = &mut self.overlay {
            slash.selected = index;
        }
        self.slash_confirm(window, cx);
    }

    pub(super) fn slash_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(Overlay::Slash(slash)) = &self.overlay else {
            return false;
        };
        let entries = self.slash_entries(slash, cx);
        let Some(entry) = entries.get(slash.selected) else {
            self.overlay = None;
            cx.notify();
            return false;
        };
        if !entry.enabled {
            return true;
        }
        let choice = entry.item.choice;
        let (block, start) = (slash.block, slash.start);
        let Some(query) = self.slash_query(cx) else {
            self.overlay = None;
            cx.notify();
            return false;
        };
        self.overlay = None;
        let end = start + 1 + query.len();
        if self.remove_text(block, start..end, window, cx) {
            self.apply_choice(choice, block, start, window, cx);
        }
        true
    }

    fn remove_text(
        &mut self,
        block: BlockId,
        range: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let start = range.start;
        let Some(edited) = self
            .block(block, cx)
            .and_then(|found| found.text.delete(range).ok())
        else {
            return false;
        };
        let command = Command::EditText {
            block,
            text: edited,
        };
        if self.dispatch(command, window, cx).is_none() {
            return false;
        }
        self.set_caret(
            TextPoint {
                block,
                offset: start,
            },
            cx,
        );
        true
    }

    fn apply_choice(
        &mut self,
        choice: Choice,
        block: BlockId,
        start: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match choice {
            Choice::Page => {
                let Some(parent) = self.block(block, cx).map(|found| found.parent) else {
                    return;
                };
                let command = Command::CreatePage {
                    parent,
                    after: Some(block),
                };
                if self.dispatch(command, window, cx).is_some() {
                    self.finish_insertion(block, start, window, cx);
                }
            }
            Choice::Database => {
                let command = Command::CreateDatabaseWithView { after: block };
                if self.dispatch(command, window, cx).is_some() {
                    self.finish_insertion(block, start, window, cx);
                }
            }
            Choice::DatabaseView => {
                self.overlay = Some(Overlay::Picker(Picker::new(block, start)));
                cx.notify();
            }
            Choice::Divider => {
                let command = Command::ChangeKind {
                    block,
                    kind: BlockKind::Divider,
                };
                if self.dispatch(command, window, cx).is_some() {
                    self.continue_after_divider(block, window, cx);
                }
            }
            other => {
                if let Some(kind) = kind_of(other) {
                    self.dispatch(Command::ChangeKind { block, kind }, window, cx);
                }
            }
        }
    }

    pub(super) fn continue_after_divider(
        &mut self,
        divider: BlockId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(parent) = self.block(divider, cx).map(|found| found.parent) else {
            return;
        };
        let following = self
            .model
            .read(cx)
            .workspace()
            .children(parent)
            .skip_while(|sibling| sibling.id != divider)
            .nth(1)
            .map(|sibling| sibling.id);
        match following {
            Some(next) => self.set_caret(
                TextPoint {
                    block: next,
                    offset: 0,
                },
                cx,
            ),
            None => {
                let command = Command::InsertBlock {
                    after: divider,
                    kind: BlockKind::Text,
                };
                self.dispatch(command, window, cx);
            }
        }
    }

    fn finish_insertion(
        &mut self,
        block: BlockId,
        start: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let removable = self
            .block(block, cx)
            .is_some_and(|found| found.text.is_empty())
            && self
                .model
                .read(cx)
                .workspace()
                .children(Parent::Block(block))
                .next()
                .is_none();
        if removable {
            self.delete_atomic(block, window, cx);
        } else {
            self.set_caret(
                TextPoint {
                    block,
                    offset: start,
                },
                cx,
            );
        }
    }

    pub(super) fn picker_entries(&self, picker: &Picker, cx: &App) -> Vec<DatabaseEntry> {
        let needle = picker.query.to_lowercase();
        self.model
            .read(cx)
            .workspace()
            .databases()
            .filter(|database| database.name.to_lowercase().contains(&needle))
            .map(|database| DatabaseEntry {
                id: database.id,
                name: if database.name.is_empty() {
                    "Untitled".to_owned()
                } else {
                    database.name.clone()
                },
                icon: database.icon.as_ref().map(|icon| icon.as_str().to_owned()),
            })
            .collect()
    }

    pub(super) fn picker_rows(&self, picker: &Picker, cx: &App) -> Vec<MenuRow> {
        self.picker_entries(picker, cx)
            .into_iter()
            .map(|entry| MenuRow {
                label: SharedString::from(entry.name),
                detail: None,
                icon: entry.icon.map(SharedString::from),
                enabled: true,
            })
            .collect()
    }

    pub(super) fn picker_step(&mut self, down: bool, cx: &mut Context<Self>) {
        let Some(Overlay::Picker(picker)) = &self.overlay else {
            return;
        };
        let count = self.picker_entries(picker, cx).len();
        if count == 0 {
            return;
        }
        let next = step_index(picker.selected, count, down, |_| true);
        if let Some(Overlay::Picker(picker)) = &mut self.overlay {
            picker.selected = next;
            picker.scroll.scroll_to_item(next);
        }
        cx.notify();
    }

    pub(super) fn picker_pick(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(Overlay::Picker(picker)) = &mut self.overlay {
            picker.selected = index;
        }
        self.picker_confirm(window, cx);
    }

    pub(super) fn picker_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::Picker(picker)) = &self.overlay else {
            return;
        };
        let (block, start) = (picker.block, picker.start);
        let chosen = self
            .picker_entries(picker, cx)
            .get(picker.selected)
            .map(|entry| entry.id);
        self.overlay = None;
        cx.notify();
        let Some(database) = chosen else {
            return;
        };
        let command = Command::InsertDatabaseView {
            after: block,
            database,
        };
        if self.dispatch(command, window, cx).is_some() {
            self.finish_insertion(block, start, window, cx);
        }
    }

    pub(super) fn picker_type(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(Overlay::Picker(picker)) = &mut self.overlay {
            picker.query.push_str(&single_line(text));
            picker.selected = 0;
            picker.scroll.scroll_to_item(0);
            cx.notify();
        }
    }

    pub(super) fn picker_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::Picker(picker)) = &mut self.overlay {
            if let Some((last, _)) = picker.query.grapheme_indices(true).next_back() {
                picker.query.truncate(last);
            }
            picker.selected = 0;
            picker.scroll.scroll_to_item(0);
            cx.notify();
        }
    }
}

pub fn single_line(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\n' | '\r')).collect()
}

#[cfg(test)]
mod tests {
    use gpui_kit::{ElementId, TestAppContext};
    use remindr_core::application::TextPoint;
    use remindr_core::domain::{BlockId, BlockKind, HeadingLevel, Parent};

    use super::filter;
    use crate::editor::testing::{Doc, Fixture};

    fn at(block: BlockId, offset: usize) -> TextPoint {
        TextPoint { block, offset }
    }

    fn labels(query: &str) -> Vec<&'static str> {
        filter(query).into_iter().map(|item| item.label).collect()
    }

    fn slash_item(index: u64) -> ElementId {
        ElementId::from(("slash-item", index))
    }

    fn picker_item(index: u64) -> ElementId {
        ElementId::from(("picker-item", index))
    }

    fn open(cx: &mut TestAppContext, doc: &Doc, block: BlockId) -> Fixture {
        let fixture = Fixture::open(cx, doc);
        fixture.focus_at(cx, at(block, 0));
        fixture
    }

    #[test]
    fn slash_items_filter_by_label_and_alias() {
        assert_eq!(
            labels(""),
            vec![
                "Text",
                "Heading 1",
                "Heading 2",
                "Heading 3",
                "Bulleted list",
                "Numbered list",
                "To-do",
                "Toggle",
                "Quote",
                "Callout",
                "Code",
                "Divider",
                "Page",
                "Database",
                "Database view",
            ]
        );
        assert_eq!(labels("tod"), vec!["To-do"]);
        assert_eq!(labels("TOD"), vec!["To-do"]);
        assert_eq!(labels("h1"), vec!["Heading 1"]);
        assert_eq!(labels("head"), vec!["Heading 1", "Heading 2", "Heading 3"]);
        assert_eq!(labels("hr"), vec!["Divider"]);
        assert_eq!(labels("board"), vec!["Database"]);
        assert_eq!(labels("linked"), vec!["Database view"]);
        assert_eq!(labels("paragraph"), vec!["Text"]);
        assert_eq!(labels("list"), vec!["Bulleted list", "Numbered list"]);
        assert_eq!(labels("database"), vec!["Database", "Database view"]);
        assert_eq!(labels("database v"), vec!["Database view"]);
        assert!(labels("zzz").is_empty());
    }

    #[test]
    fn prefix_matches_come_before_substring_matches() {
        assert_eq!(
            labels("d"),
            vec![
                "Divider",
                "Database",
                "Database view",
                "Heading 1",
                "Heading 2",
                "Heading 3",
                "Bulleted list",
                "Numbered list",
                "To-do",
                "Code",
            ]
        );
    }

    #[gpui_kit::test]
    fn typing_slash_tod_then_enter_makes_a_todo(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/tod");
        assert!(fixture.shown(cx, "slash-menu"));
        assert!(fixture.shown(cx, slash_item(0)));
        assert!(!fixture.shown(cx, slash_item(1)));
        assert_eq!(fixture.text(cx, block), "/tod");
        fixture.press(cx, "enter");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::ToDo { checked: false })
        );
        assert_eq!(fixture.text(cx, block), "");
        assert_eq!(fixture.caret(cx), Some(at(block, 0)));
        assert!(!fixture.shown(cx, "slash-menu"));
    }

    #[gpui_kit::test]
    fn the_query_is_the_text_between_the_slash_and_the_caret(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "note ");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 5));
        fixture.input(cx, "/h2");
        fixture.press(cx, "enter");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::Heading(HeadingLevel::Two))
        );
        assert_eq!(fixture.text(cx, block), "note ");
        assert_eq!(fixture.caret(cx), Some(at(block, 5)));
    }

    #[gpui_kit::test]
    fn arrow_keys_move_the_selection_and_enter_applies_it(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/head");
        fixture.press(cx, "down");
        fixture.press(cx, "down");
        fixture.press(cx, "up");
        fixture.press(cx, "enter");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::Heading(HeadingLevel::Two))
        );
        assert_eq!(fixture.caret(cx), Some(at(block, 0)));
    }

    #[gpui_kit::test]
    fn clicking_an_item_applies_it(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;

        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/");
        fixture.with(cx, |window, cx| window.click(slash_item(7), cx));
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Toggle));
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn escape_keeps_the_slash_text(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/to");
        assert!(fixture.shown(cx, "slash-menu"));
        fixture.press(cx, "escape");
        assert!(!fixture.shown(cx, "slash-menu"));
        assert_eq!(fixture.text(cx, block), "/to");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Text));
        fixture.press(cx, "enter");
        assert_eq!(fixture.texts(cx), vec!["/to".to_owned(), String::new()]);
    }

    #[gpui_kit::test]
    fn a_space_after_the_bare_slash_closes_the_menu(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/");
        assert!(fixture.shown(cx, "slash-menu"));
        fixture.input(cx, " ");
        assert!(!fixture.shown(cx, "slash-menu"));
        assert_eq!(fixture.text(cx, block), "/ ");
    }

    #[gpui_kit::test]
    fn moving_the_caret_away_closes_the_menu(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/to");
        assert!(fixture.shown(cx, "slash-menu"));
        fixture.press(cx, "left");
        fixture.press(cx, "left");
        fixture.press(cx, "left");
        assert!(!fixture.shown(cx, "slash-menu"));
        fixture.input(cx, "x");
        assert_eq!(fixture.text(cx, block), "x/to");
    }

    #[gpui_kit::test]
    fn backspacing_the_slash_closes_the_menu(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/t");
        fixture.press(cx, "backspace");
        assert!(fixture.shown(cx, "slash-menu"));
        fixture.press(cx, "backspace");
        assert!(!fixture.shown(cx, "slash-menu"));
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn no_match_shows_a_row_in_the_menu(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/zzz");
        assert!(fixture.shown(cx, "slash-menu"));
        assert!(fixture.shown(cx, "slash-empty"));
        assert!(!fixture.shown(cx, slash_item(0)));
        assert_eq!(fixture.text(cx, block), "/zzz");
    }

    #[gpui_kit::test]
    fn enter_with_no_results_closes_the_menu_and_splits(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/zzz");
        assert!(fixture.shown(cx, "slash-empty"));
        fixture.press(cx, "enter");
        assert!(!fixture.shown(cx, "slash-menu"));
        assert_eq!(fixture.texts(cx), vec!["/zzz".to_owned(), String::new()]);
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Text));
        let caret = fixture.caret(cx).expect("caret");
        assert_ne!(caret.block, block);
        assert_eq!(caret.offset, 0);
    }

    #[gpui_kit::test]
    fn enter_on_a_disabled_item_keeps_the_menu_and_does_not_split(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/linked");
        fixture.press(cx, "enter");
        assert!(fixture.shown(cx, "slash-menu"));
        assert_eq!(fixture.texts(cx), vec!["/linked".to_owned()]);
    }

    #[gpui_kit::test]
    fn slash_inside_a_word_does_not_open_the_menu(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "and/or");
        assert_eq!(fixture.text(cx, block), "and/or");
        assert!(!fixture.shown(cx, "slash-menu"));
        fixture.input(cx, " see https://example.com");
        assert_eq!(fixture.text(cx, block), "and/or see https://example.com");
        assert!(!fixture.shown(cx, "slash-menu"));
        fixture.press(cx, "enter");
        assert_eq!(fixture.texts(cx).len(), 2);
    }

    #[gpui_kit::test]
    fn slash_after_a_space_opens_the_menu(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "and /");
        assert!(fixture.shown(cx, "slash-menu"));
        fixture.input(cx, "h1");
        fixture.press(cx, "enter");
        assert_eq!(
            fixture.kind(cx, block),
            Some(BlockKind::Heading(HeadingLevel::One))
        );
        assert_eq!(fixture.text(cx, block), "and ");
    }

    #[gpui_kit::test]
    fn slash_after_a_soft_break_opens_the_menu(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "line");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 4));
        fixture.press(cx, "shift-enter");
        fixture.input(cx, "/");
        assert!(fixture.shown(cx, "slash-menu"));
    }

    #[gpui_kit::test]
    fn slash_at_the_start_of_existing_text_opens_the_menu(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "tail");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 0));
        fixture.input(cx, "/");
        assert!(fixture.shown(cx, "slash-menu"));
        assert_eq!(fixture.text(cx, block), "/tail");
    }

    #[gpui_kit::test]
    fn the_menu_shows_at_most_eight_rows_and_scrolls_with_the_selection(cx: &mut TestAppContext) {
        use gpui_kit::px;
        use gpui_kit::test::TestWindowExt;

        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/");
        let menu = fixture.with(cx, |window, _| window.find("slash-menu").bounds());
        assert_eq!(menu.size.width, px(320.));
        let rows: Vec<_> = fixture.with(cx, |window, _| {
            (0..15)
                .filter_map(|index| window.try_find(slash_item(index)))
                .map(|found| found.bounds())
                .collect()
        });
        assert_eq!(rows.len(), 15);
        let height = rows[0].size.height;
        assert!(menu.size.height < height * 9.);
        assert!(menu.size.height >= height * 8.);
        for _ in 0..12 {
            fixture.press(cx, "down");
        }
        let (menu, last) = fixture.with(cx, |window, _| {
            (
                window.find("slash-menu").bounds(),
                window.find(slash_item(12)).bounds(),
            )
        });
        assert!(last.bottom() <= menu.bottom() + px(1.));
        assert!(last.top() >= menu.top() - px(1.));
    }

    #[gpui_kit::test]
    fn the_menu_sits_under_the_caret_line(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;

        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/");
        let (menu, row) = fixture.with(cx, |window, _| {
            (
                window.find("slash-menu").bounds(),
                window.find(crate::editor::render::row_id(block)).bounds(),
            )
        });
        assert!(menu.top() >= row.bottom() - gpui_kit::px(8.));
        assert!(menu.top() <= row.bottom() + gpui_kit::px(16.));
    }

    #[gpui_kit::test]
    fn the_menu_flips_above_when_it_would_overflow_the_window(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;

        let mut doc = Doc::new();
        for number in 0..30 {
            doc.child(BlockKind::Text, &format!("line {number}"));
        }
        let last = doc.child(BlockKind::Text, "");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(last, 0));
        fixture.with(cx, |window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
        });
        fixture.input(cx, "/");
        let (menu, row) = fixture.with(cx, |window, _| {
            (
                window.find("slash-menu").bounds(),
                window.find(crate::editor::render::row_id(last)).bounds(),
            )
        });
        assert!(menu.bottom() <= row.top() + gpui_kit::px(8.));
        assert!(menu.top() >= gpui_kit::px(0.));
    }

    #[gpui_kit::test]
    fn slash_page_creates_a_sub_page_block(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/page");
        fixture.press(cx, "enter");
        assert_eq!(
            fixture.children_kinds(cx, fixture.page),
            vec![BlockKind::Page { icon: None }]
        );
        assert!(!fixture.alive(cx, block));
        let page = fixture.children_ids(cx, fixture.page)[0];
        assert_eq!(fixture.parent(cx, page), Some(Parent::Block(fixture.page)));
        assert!(fixture.caret(cx).is_some());
    }

    #[gpui_kit::test]
    fn slash_page_after_text_keeps_the_text_block(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "intro ");
        let tail = doc.child(BlockKind::Text, "tail");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 6));
        fixture.input(cx, "/page");
        fixture.press(cx, "enter");
        assert_eq!(fixture.text(cx, block), "intro ");
        let children = fixture.children_ids(cx, fixture.page);
        assert_eq!(children.len(), 3);
        assert_eq!(children[0], block);
        assert_eq!(children[2], tail);
        assert_eq!(
            fixture.kind(cx, children[1]),
            Some(BlockKind::Page { icon: None })
        );
        assert_eq!(fixture.caret(cx), Some(at(block, 6)));
    }

    #[gpui_kit::test]
    fn slash_page_inside_a_toggle_creates_the_page_in_the_toggle(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let toggle = doc.child(BlockKind::Toggle, "T");
        let inner = doc.under(toggle, BlockKind::Text, "");
        let fixture = Fixture::open(cx, &doc);
        fixture.with(cx, |window, cx| {
            use gpui_kit::test::TestWindowExt;
            window.click(crate::editor::render::chevron_id(toggle), cx);
        });
        fixture.focus_at(cx, at(inner, 0));
        fixture.input(cx, "/page");
        fixture.press(cx, "enter");
        assert_eq!(
            fixture.children_kinds(cx, toggle),
            vec![BlockKind::Page { icon: None }]
        );
    }

    #[gpui_kit::test]
    fn slash_database_inserts_a_view_block(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/database");
        fixture.press(cx, "enter");
        assert_eq!(fixture.database_count(cx), 1);
        let kinds = fixture.children_kinds(cx, fixture.page);
        assert_eq!(kinds.len(), 1);
        assert!(matches!(kinds[0], BlockKind::DatabaseView { .. }));
        assert!(!fixture.alive(cx, block));
    }

    #[gpui_kit::test]
    fn slash_database_view_lists_existing_databases(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        let sales = fixture.create_database(cx, "Sales");
        let tasks = fixture.create_database(cx, "Tasks");
        fixture.input(cx, "/database view");
        fixture.press(cx, "enter");
        assert!(!fixture.shown(cx, "slash-menu"));
        assert!(fixture.shown(cx, "database-picker"));
        assert_eq!(
            fixture.label_of(cx, picker_item(0)).as_deref(),
            Some("Sales")
        );
        assert_eq!(
            fixture.label_of(cx, picker_item(1)).as_deref(),
            Some("Tasks")
        );
        assert!(!fixture.shown(cx, picker_item(2)));
        assert_eq!(fixture.text(cx, block), "");
        fixture.input(cx, "tas");
        assert!(!fixture.shown(cx, picker_item(1)));
        assert_eq!(
            fixture.label_of(cx, picker_item(0)).as_deref(),
            Some("Tasks")
        );
        fixture.press(cx, "enter");
        assert!(!fixture.shown(cx, "database-picker"));
        let kinds = fixture.children_kinds(cx, fixture.page);
        assert_eq!(kinds, vec![BlockKind::DatabaseView { database: tasks }]);
        assert_ne!(sales, tasks);
        assert!(!fixture.alive(cx, block));
    }

    #[gpui_kit::test]
    fn the_database_picker_closes_on_escape_without_inserting(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.create_database(cx, "Sales");
        fixture.input(cx, "/linked");
        fixture.press(cx, "enter");
        assert!(fixture.shown(cx, "database-picker"));
        fixture.press(cx, "escape");
        assert!(!fixture.shown(cx, "database-picker"));
        assert_eq!(
            fixture.children_kinds(cx, fixture.page),
            vec![BlockKind::Text]
        );
    }

    #[gpui_kit::test]
    fn slash_database_view_is_disabled_without_a_database(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/linked");
        assert_eq!(
            fixture.label_of(cx, slash_item(0)).as_deref(),
            Some("Database view, No database yet")
        );
        fixture.press(cx, "enter");
        assert!(!fixture.shown(cx, "database-picker"));
        assert_eq!(
            fixture.children_kinds(cx, fixture.page),
            vec![BlockKind::Text]
        );
        assert_eq!(fixture.text(cx, block), "/linked");
    }

    #[gpui_kit::test]
    fn slash_code_and_callout_use_their_defaults(cx: &mut TestAppContext) {
        use remindr_core::domain::{CodeLanguage, Emoji};

        let mut doc = Doc::new();
        let code = doc.child(BlockKind::Text, "");
        let callout = doc.child(BlockKind::Text, "");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(code, 0));
        fixture.input(cx, "/code");
        fixture.press(cx, "enter");
        assert_eq!(
            fixture.kind(cx, code),
            Some(BlockKind::Code {
                language: CodeLanguage::plain_text()
            })
        );
        fixture.focus_at(cx, at(callout, 0));
        fixture.input(cx, "/callout");
        fixture.press(cx, "enter");
        assert_eq!(
            fixture.kind(cx, callout),
            Some(BlockKind::Callout {
                icon: Emoji::try_from("💡").expect("emoji")
            })
        );
    }

    #[gpui_kit::test]
    fn slash_divider_leaves_a_text_block_to_continue_in(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/divider");
        fixture.press(cx, "enter");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Divider));
        let children = fixture.children_ids(cx, fixture.page);
        assert_eq!(children.len(), 2);
        assert_eq!(fixture.kind(cx, children[1]), Some(BlockKind::Text));
        assert_eq!(fixture.caret(cx), Some(at(children[1], 0)));
    }

    #[gpui_kit::test]
    fn undo_of_a_slash_choice_restores_the_kind(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.input(cx, "/quote");
        fixture.press(cx, "enter");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Quote));
        fixture.press(cx, "secondary-z");
        assert_eq!(fixture.kind(cx, block), Some(BlockKind::Text));
    }

    #[gpui_kit::test]
    fn clicking_a_database_in_the_picker_inserts_it(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;

        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.create_database(cx, "Sales");
        let tasks = fixture.create_database(cx, "Tasks");
        fixture.input(cx, "/linked");
        fixture.press(cx, "enter");
        fixture.with(cx, |window, cx| window.click(picker_item(1), cx));
        assert_eq!(
            fixture.children_kinds(cx, fixture.page),
            vec![BlockKind::DatabaseView { database: tasks }]
        );
    }

    #[gpui_kit::test]
    fn the_picker_moves_with_the_arrow_keys_and_wraps(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        let sales = fixture.create_database(cx, "Sales");
        let tasks = fixture.create_database(cx, "Tasks");
        fixture.input(cx, "/linked");
        fixture.press(cx, "enter");
        fixture.press(cx, "up");
        fixture.press(cx, "down");
        fixture.press(cx, "down");
        fixture.press(cx, "down");
        fixture.press(cx, "up");
        fixture.press(cx, "up");
        fixture.press(cx, "enter");
        let kinds = fixture.children_kinds(cx, fixture.page);
        assert_eq!(kinds, vec![BlockKind::DatabaseView { database: sales }]);
        assert_ne!(sales, tasks);
    }

    #[gpui_kit::test]
    fn the_picker_says_so_when_no_database_matches(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.create_database(cx, "Sales");
        fixture.input(cx, "/linked");
        fixture.press(cx, "enter");
        fixture.input(cx, "zzz");
        assert!(fixture.shown(cx, "picker-empty"));
        fixture.press(cx, "enter");
        assert!(!fixture.shown(cx, "database-picker"));
        assert_eq!(
            fixture.children_kinds(cx, fixture.page),
            vec![BlockKind::Text]
        );
    }

    #[gpui_kit::test]
    fn the_picker_filter_takes_backspace(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = open(cx, &doc, block);
        fixture.create_database(cx, "Sales");
        fixture.create_database(cx, "Tasks");
        fixture.input(cx, "/linked");
        fixture.press(cx, "enter");
        fixture.input(cx, "tx");
        assert!(fixture.shown(cx, "picker-empty"));
        fixture.press(cx, "backspace");
        assert_eq!(
            fixture.label_of(cx, picker_item(0)).as_deref(),
            Some("Tasks")
        );
        assert_eq!(fixture.text(cx, block), "");
    }

    #[gpui_kit::test]
    fn clicking_in_the_page_closes_the_picker(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;

        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let other = doc.child(BlockKind::Text, "other");
        let fixture = open(cx, &doc, block);
        fixture.create_database(cx, "Sales");
        fixture.input(cx, "/linked");
        fixture.press(cx, "enter");
        assert!(fixture.shown(cx, "database-picker"));
        fixture.with(cx, |window, cx| {
            window.click(crate::editor::render::row_id(other), cx);
        });
        assert!(!fixture.shown(cx, "database-picker"));
        assert_eq!(fixture.caret(cx).map(|caret| caret.block), Some(other));
    }

    #[gpui_kit::test]
    fn slash_menu_works_inside_a_nested_block_and_restores_on_undo(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let parent = doc.child(BlockKind::Text, "parent");
        let child = doc.under(parent, BlockKind::Text, "");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(child, 0));
        fixture.input(cx, "/h1");
        fixture.press(cx, "enter");
        assert_eq!(
            fixture.kind(cx, child),
            Some(BlockKind::Heading(HeadingLevel::One))
        );
        assert_eq!(fixture.parent(cx, child), Some(Parent::Block(parent)));
    }
}
