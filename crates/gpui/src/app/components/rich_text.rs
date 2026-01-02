use std::{ops::Range, time::Duration};

use gpui::{
    App, Bounds, ClipboardItem, Context, ElementInputHandler, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, FontStyle, FontWeight, HighlightStyle,
    InteractiveElement, IntoElement, KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Refineable, RenderOnce,
    SharedString, StrikethroughStyle, StyleRefinement, Styled, Task, Timer, UTF16Selection,
    UnderlineStyle, Window, actions, canvas, div, prelude::FluentBuilder, px,
};
use gpui_component::{ActiveTheme, menu::ContextMenuExt};
use serde::{Deserialize, Serialize};

// Actions for keyboard handling
actions!(
    rich_text,
    [
        Backspace,
        Delete,
        Enter,
        Tab,
        Space,
        Slash,
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        MoveToStart,
        MoveToEnd,
        MoveWordLeft,
        MoveWordRight,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        ToggleBold,
        ToggleItalic,
        ToggleUnderline,
        ToggleStrikethrough,
        ToggleCode,
        ShowCharacterPalette,
    ]
);

const CONTEXT: &str = "RichText";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
        KeyBinding::new("delete", Delete, Some(CONTEXT)),
        KeyBinding::new("enter", Enter, Some(CONTEXT)),
        KeyBinding::new("tab", Tab, Some(CONTEXT)),
        KeyBinding::new("space", Space, Some(CONTEXT)),
        KeyBinding::new("/", Slash, Some(CONTEXT)),
        KeyBinding::new("left", MoveLeft, Some(CONTEXT)),
        KeyBinding::new("right", MoveRight, Some(CONTEXT)),
        KeyBinding::new("up", MoveUp, Some(CONTEXT)),
        KeyBinding::new("down", MoveDown, Some(CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-left", MoveWordLeft, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-right", MoveWordRight, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-shift-left", SelectWordLeft, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-shift-right", SelectWordRight, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-left", MoveWordLeft, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-right", MoveWordRight, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-right", SelectWordRight, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", Copy, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-x", Cut, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-x", Cut, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", Paste, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", Paste, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-z", Undo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-z", Undo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-z", Redo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-y", Redo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-b", ToggleBold, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-b", ToggleBold, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-i", ToggleItalic, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-i", ToggleItalic, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-u", ToggleUnderline, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-u", ToggleUnderline, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-up", MoveToStart, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-down", MoveToEnd, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some(CONTEXT)),
    ]);
}

/// Text formatting style
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RichTextStyle {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
}

/// A span of styled text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
    pub style: RichTextStyle,
}

impl TextSpan {
    pub fn new(start: usize, end: usize, style: RichTextStyle) -> Self {
        Self { start, end, style }
    }

    pub fn overlaps(&self, start: usize, end: usize) -> bool {
        self.start < end && self.end > start
    }

    pub fn contains(&self, start: usize, end: usize) -> bool {
        self.start <= start && self.end >= end
    }
}

/// Events emitted by RichText
#[derive(Clone)]
pub enum RichTextEvent {
    Change(SharedString),
    Focus,
    Blur,
    Enter,
    Tab,
    Backspace,
    Delete,
    Space,
    Slash,
}

/// Selection in the text
#[derive(Debug, Clone, Copy, Default)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
}

impl Selection {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn cursor(pos: usize) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn len(&self) -> usize {
        if self.start <= self.end {
            self.end - self.start
        } else {
            self.start - self.end
        }
    }

    pub fn normalized(&self) -> (usize, usize) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    pub fn head(&self) -> usize {
        self.end
    }

    pub fn anchor(&self) -> usize {
        self.start
    }
}

const BLINK_INTERVAL: Duration = Duration::from_millis(500);
const BLINK_PAUSE_DELAY: Duration = Duration::from_millis(300);

/// Manages cursor blinking animation
pub struct BlinkCursor {
    visible: bool,
    paused: bool,
    epoch: usize,
    _task: Task<()>,
}

impl BlinkCursor {
    pub fn new() -> Self {
        Self {
            visible: true,
            paused: false,
            epoch: 0,
            _task: Task::ready(()),
        }
    }

    pub fn start(&mut self, cx: &mut Context<RichTextState>) {
        self.visible = true;
        self.blink(self.epoch, cx);
    }

    pub fn stop(&mut self, cx: &mut Context<RichTextState>) {
        self.epoch = self.epoch.wrapping_add(1);
        self.visible = false;
        self._task = Task::ready(());
        cx.notify();
    }

    fn next_epoch(&mut self) -> usize {
        self.epoch = self.epoch.wrapping_add(1);
        self.epoch
    }

    fn blink(&mut self, epoch: usize, cx: &mut Context<RichTextState>) {
        if self.paused || epoch != self.epoch {
            self.visible = true;
            return;
        }

        self.visible = !self.visible;
        cx.notify();

        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            Timer::after(BLINK_INTERVAL).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |state, cx| state.blink_cursor.blink(epoch, cx))
                    .ok();
            }
        });
    }

    pub fn visible(&self) -> bool {
        self.paused || self.visible
    }

    pub fn pause(&mut self, cx: &mut Context<RichTextState>) {
        self.paused = true;
        self.visible = true;
        cx.notify();

        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            Timer::after(BLINK_PAUSE_DELAY).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |state, cx| {
                    state.blink_cursor.paused = false;
                    state.blink_cursor.blink(epoch, cx);
                })
                .ok();
            }
        });
    }
}

/// State for the RichText component
pub struct RichTextState {
    pub focus_handle: FocusHandle,
    content: String,
    spans: Vec<TextSpan>,
    selection: Selection,
    blink_cursor: BlinkCursor,
    is_selecting: bool,
    last_bounds: Option<Bounds<Pixels>>,
    history: Vec<(String, Vec<TextSpan>, Selection)>,
    history_index: usize,
    marked_range: Option<Range<usize>>,
    wrapped_line_count: usize,
}

impl EventEmitter<RichTextEvent> for RichTextState {}

impl RichTextState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        cx.on_focus(&focus_handle, window, Self::on_focus).detach();
        cx.on_blur(&focus_handle, window, Self::on_blur).detach();

        Self {
            focus_handle,
            content: String::new(),
            spans: Vec::new(),
            selection: Selection::default(),
            blink_cursor: BlinkCursor::new(),
            is_selecting: false,
            last_bounds: None,
            history: vec![(String::new(), Vec::new(), Selection::default())],
            wrapped_line_count: 1,
            history_index: 0,
            marked_range: None,
        }
    }

    fn on_focus(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.blink_cursor.start(cx);
        cx.emit(RichTextEvent::Focus);
    }

    fn on_blur(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.blink_cursor.stop(cx);
        cx.emit(RichTextEvent::Blur);
    }

    pub fn cursor_visible(&self) -> bool {
        self.blink_cursor.visible()
    }

    pub fn is_selecting(&self) -> bool {
        self.is_selecting
    }

    pub fn set_bounds(&mut self, bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        let width_changed = self
            .last_bounds
            .map(|b| b.size.width != bounds.size.width)
            .unwrap_or(true);
        self.last_bounds = Some(bounds);
        if width_changed {
            cx.notify();
        }
    }

    pub fn last_bounds(&self) -> Option<Bounds<Pixels>> {
        self.last_bounds
    }

    pub fn set_wrapped_line_count(&mut self, count: usize) {
        self.wrapped_line_count = count.max(1);
    }

    pub fn wrapped_line_count(&self) -> usize {
        self.wrapped_line_count
    }

    /// Calculate cursor position from mouse point (x, y) with wrap support
    pub fn position_from_point(
        &self,
        point: Point<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = font_size * 1.5;

        // Get wrap width from bounds
        let wrap_width = self.last_bounds.map(|b| b.size.width).unwrap_or(px(1000.0));

        // Calculate wrap boundaries manually by measuring text width
        let mut wrap_boundaries: Vec<usize> = Vec::new();
        let mut current_line_start = 0;

        // Find word boundaries and calculate where lines wrap
        let mut last_space_idx = 0;
        for (idx, ch) in self.content.char_indices() {
            // Store the byte index AFTER the space character
            if ch.is_whitespace() {
                last_space_idx = idx + ch.len_utf8();
            }

            // Use the byte index AFTER the current character (idx + char byte length)
            let char_end = idx + ch.len_utf8();
            let text_since_line_start = &self.content[current_line_start..char_end];
            let width = window
                .text_system()
                .shape_line(
                    SharedString::from(text_since_line_start.to_string()),
                    font_size,
                    &[text_style.to_run(text_since_line_start.len())],
                    None,
                )
                .width;

            if width > wrap_width && idx > current_line_start {
                // Wrap at the last space if possible, otherwise at current position
                let wrap_at = if last_space_idx > current_line_start {
                    last_space_idx // Already points after the space
                } else {
                    idx
                };
                wrap_boundaries.push(wrap_at);
                current_line_start = wrap_at;
                last_space_idx = wrap_at;
            }
        }

        // Determine which visual line was clicked based on Y
        let clicked_line = (point.y / line_height).floor().max(0.0) as usize;

        // Calculate the byte range for the clicked line
        let (line_start_byte, line_end_byte) = if wrap_boundaries.is_empty() {
            // No wrapping, single line
            (0, self.content.len())
        } else {
            let line_start = if clicked_line == 0 {
                0
            } else if clicked_line <= wrap_boundaries.len() {
                wrap_boundaries[clicked_line - 1]
            } else {
                *wrap_boundaries.last().unwrap_or(&0)
            };

            let line_end = if clicked_line < wrap_boundaries.len() {
                wrap_boundaries[clicked_line]
            } else {
                self.content.len()
            };

            (line_start, line_end)
        };

        // Find the position within this line
        let x = point.x;

        // Search within the line's byte range
        let mut best_pos = line_start_byte;
        let mut best_distance = x.abs();

        // Get text for this line
        let line_text = &self.content[line_start_byte..line_end_byte];

        for (idx, _) in line_text.char_indices() {
            let abs_idx = line_start_byte + idx;
            let text_in_line = &line_text[..idx];

            let width = if !text_in_line.is_empty() {
                let len = text_in_line.len();
                window
                    .text_system()
                    .shape_line(
                        SharedString::from(text_in_line.to_string()),
                        font_size,
                        &[text_style.to_run(len)],
                        None,
                    )
                    .width
            } else {
                px(0.0)
            };

            let distance = (width - x).abs();
            if distance < best_distance {
                best_distance = distance;
                best_pos = abs_idx;
            }
        }

        // Also check the end of the line
        let line_width = if !line_text.is_empty() {
            window
                .text_system()
                .shape_line(
                    SharedString::from(line_text.to_string()),
                    font_size,
                    &[text_style.to_run(line_text.len())],
                    None,
                )
                .width
        } else {
            px(0.0)
        };

        if (line_width - x).abs() < best_distance {
            best_pos = line_end_byte;
        }

        best_pos
    }

    /// Find word boundaries at a given position
    fn word_bounds_at(&self, pos: usize) -> (usize, usize) {
        if self.content.is_empty() {
            return (0, 0);
        }

        let pos = pos.min(self.content.len());

        // Find word start (go backwards until we hit a non-word character)
        let mut start = pos;
        for (idx, ch) in self.content[..pos].char_indices().rev() {
            if !ch.is_alphanumeric() && ch != '_' {
                start = idx + ch.len_utf8();
                break;
            }
            start = idx;
        }

        // Find word end (go forwards until we hit a non-word character)
        let mut end = pos;
        for (idx, ch) in self.content[pos..].char_indices() {
            if !ch.is_alphanumeric() && ch != '_' {
                end = pos + idx;
                break;
            }
            end = pos + idx + ch.len_utf8();
        }

        (start, end)
    }

    pub fn handle_mouse_down(
        &mut self,
        position: Point<Pixels>,
        click_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        self.blink_cursor.pause(cx);

        // Convert absolute position to relative position
        let relative_point = if let Some(bounds) = self.last_bounds {
            Point {
                x: position.x - bounds.origin.x,
                y: position.y - bounds.origin.y,
            }
        } else {
            position
        };

        let cursor_pos = self.position_from_point(relative_point, window, cx);

        match click_count {
            2 => {
                // Double-click: select word
                let (word_start, word_end) = self.word_bounds_at(cursor_pos);
                self.selection = Selection::new(word_start, word_end);
            }
            3 => {
                // Triple-click: select all
                self.selection = Selection::new(0, self.content.len());
            }
            _ => {
                // Single click: position cursor
                self.selection = Selection::cursor(cursor_pos);
            }
        }

        cx.notify();
    }

    pub fn handle_mouse_move(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_selecting {
            return;
        }

        // Convert absolute position to relative position
        let relative_point = if let Some(bounds) = self.last_bounds {
            Point {
                x: position.x - bounds.origin.x,
                y: position.y - bounds.origin.y,
            }
        } else {
            position
        };

        let cursor_pos = self.position_from_point(relative_point, window, cx);
        self.selection.end = cursor_pos;
        cx.notify();
    }

    pub fn handle_mouse_up(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.is_selecting = false;
        cx.notify();
    }

    pub fn set_content(&mut self, content: impl Into<String>, cx: &mut Context<Self>) {
        self.content = content.into();
        self.selection = Selection::cursor(self.content.len());
        self.spans.clear();
        self.push_history();
        cx.notify();
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn value(&self) -> SharedString {
        SharedString::from(self.content.clone())
    }

    pub fn spans(&self) -> &[TextSpan] {
        &self.spans
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn set_selection(&mut self, selection: Selection, cx: &mut Context<Self>) {
        self.selection = selection;
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, _cx: &mut Context<Self>) {
        self.focus_handle.focus(window);
    }

    fn push_history(&mut self) {
        // Remove future history if we're not at the end
        if self.history_index < self.history.len() - 1 {
            self.history.truncate(self.history_index + 1);
        }
        self.history
            .push((self.content.clone(), self.spans.clone(), self.selection));
        self.history_index = self.history.len() - 1;

        // Limit history size
        if self.history.len() > 100 {
            self.history.remove(0);
            self.history_index = self.history_index.saturating_sub(1);
        }
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let (content, spans, selection) = self.history[self.history_index].clone();
            self.content = content;
            self.spans = spans;
            self.selection = selection;
            cx.emit(RichTextEvent::Change(self.value()));
            cx.notify();
        }
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            let (content, spans, selection) = self.history[self.history_index].clone();
            self.content = content;
            self.spans = spans;
            self.selection = selection;
            cx.emit(RichTextEvent::Change(self.value()));
            cx.notify();
        }
    }

    pub fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.blink_cursor.pause(cx);

        let (start, end) = self.selection.normalized();

        // Delete selected text if any
        if start != end {
            self.delete_range(start, end);
        }

        // Insert new text
        self.content.insert_str(start, text);

        // Update spans
        let insert_len = text.len();
        for span in &mut self.spans {
            if span.start >= start {
                span.start += insert_len;
                span.end += insert_len;
            } else if span.end > start {
                span.end += insert_len;
            }
        }

        self.selection = Selection::cursor(start + insert_len);
        self.push_history();
        cx.emit(RichTextEvent::Change(self.value()));
        cx.notify();
    }

    fn delete_range(&mut self, start: usize, end: usize) {
        let delete_len = end - start;
        self.content.replace_range(start..end, "");

        // Update spans
        self.spans.retain_mut(|span| {
            if span.end <= start {
                // Span is before deletion, keep as is
                true
            } else if span.start >= end {
                // Span is after deletion, shift left
                span.start -= delete_len;
                span.end -= delete_len;
                true
            } else if span.start >= start && span.end <= end {
                // Span is completely within deletion, remove
                false
            } else if span.start < start && span.end > end {
                // Deletion is within span, shrink span
                span.end -= delete_len;
                true
            } else if span.start < start {
                // Span overlaps start of deletion
                span.end = start;
                span.start < span.end
            } else {
                // Span overlaps end of deletion
                span.start = start;
                span.end -= delete_len;
                span.start < span.end
            }
        });
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        self.blink_cursor.pause(cx);

        let (start, end) = self.selection.normalized();

        if start != end {
            self.delete_range(start, end);
            self.selection = Selection::cursor(start);
        } else if start > 0 {
            // Find the previous character boundary
            let prev_pos = self.content[..start]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.delete_range(prev_pos, start);
            self.selection = Selection::cursor(prev_pos);
        }

        self.push_history();
        cx.emit(RichTextEvent::Change(self.value()));
        cx.emit(RichTextEvent::Backspace);
        cx.notify();
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        self.blink_cursor.pause(cx);

        let (start, end) = self.selection.normalized();

        if start != end {
            self.delete_range(start, end);
            self.selection = Selection::cursor(start);
        } else if start < self.content.len() {
            // Find the next character boundary
            let next_pos = self.content[start..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| start + i)
                .unwrap_or(self.content.len());
            self.delete_range(start, next_pos);
        }

        self.push_history();
        cx.emit(RichTextEvent::Change(self.value()));
        cx.emit(RichTextEvent::Delete);
        cx.notify();
    }

    fn move_left(&mut self, extend_selection: bool, cx: &mut Context<Self>) {
        let pos = self.selection.head();
        if pos > 0 {
            let new_pos = self.content[..pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);

            if extend_selection {
                self.selection.end = new_pos;
            } else {
                self.selection = Selection::cursor(new_pos);
            }
            cx.notify();
        } else if !extend_selection && !self.selection.is_empty() {
            let (start, _) = self.selection.normalized();
            self.selection = Selection::cursor(start);
            cx.notify();
        }
    }

    fn move_right(&mut self, extend_selection: bool, cx: &mut Context<Self>) {
        let pos = self.selection.head();
        if pos < self.content.len() {
            let new_pos = self.content[pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| pos + i)
                .unwrap_or(self.content.len());

            if extend_selection {
                self.selection.end = new_pos;
            } else {
                self.selection = Selection::cursor(new_pos);
            }
            cx.notify();
        } else if !extend_selection && !self.selection.is_empty() {
            let (_, end) = self.selection.normalized();
            self.selection = Selection::cursor(end);
            cx.notify();
        }
    }

    fn move_word_left(&mut self, extend_selection: bool, cx: &mut Context<Self>) {
        let pos = self.selection.head();
        if pos > 0 {
            let new_pos = self.find_word_boundary_left(pos);

            if extend_selection {
                self.selection.end = new_pos;
            } else {
                self.selection = Selection::cursor(new_pos);
            }
            cx.notify();
        }
    }

    fn move_word_right(&mut self, extend_selection: bool, cx: &mut Context<Self>) {
        let pos = self.selection.head();
        if pos < self.content.len() {
            let new_pos = self.find_word_boundary_right(pos);

            if extend_selection {
                self.selection.end = new_pos;
            } else {
                self.selection = Selection::cursor(new_pos);
            }
            cx.notify();
        }
    }

    /// Find the start of the previous word
    fn find_word_boundary_left(&self, pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }

        let before = &self.content[..pos];
        let chars: Vec<(usize, char)> = before.char_indices().collect();

        if chars.is_empty() {
            return 0;
        }

        let mut i = chars.len() - 1;

        // Skip trailing whitespace
        while i > 0 && chars[i].1.is_whitespace() {
            i -= 1;
        }

        // Skip word characters until we hit whitespace or start
        while i > 0 && !chars[i - 1].1.is_whitespace() {
            i -= 1;
        }

        chars.get(i).map(|(idx, _)| *idx).unwrap_or(0)
    }

    /// Find the end of the next word
    fn find_word_boundary_right(&self, pos: usize) -> usize {
        if pos >= self.content.len() {
            return self.content.len();
        }

        let after = &self.content[pos..];
        let chars: Vec<(usize, char)> = after.char_indices().collect();

        if chars.is_empty() {
            return self.content.len();
        }

        let mut i = 0;

        // Skip leading whitespace
        while i < chars.len() && chars[i].1.is_whitespace() {
            i += 1;
        }

        // Skip word characters until we hit whitespace or end
        while i < chars.len() && !chars[i].1.is_whitespace() {
            i += 1;
        }

        if i < chars.len() {
            pos + chars[i].0
        } else {
            self.content.len()
        }
    }

    fn move_to_start(&mut self, cx: &mut Context<Self>) {
        self.selection = Selection::cursor(0);
        cx.notify();
    }

    pub fn move_to_end(&mut self, cx: &mut Context<Self>) {
        self.selection = Selection::cursor(self.content.len());
        cx.notify();
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        self.selection = Selection::new(0, self.content.len());
        cx.notify();
    }

    fn copy(&self, cx: &mut Context<Self>) {
        let (start, end) = self.selection.normalized();
        if start != end {
            let text = &self.content[start..end];
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        }
    }

    fn cut(&mut self, cx: &mut Context<Self>) {
        self.copy(cx);
        let (start, end) = self.selection.normalized();
        if start != end {
            self.delete_range(start, end);
            self.selection = Selection::cursor(start);
            self.push_history();
            cx.emit(RichTextEvent::Change(self.value()));
            cx.notify();
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(clipboard) = cx.read_from_clipboard() {
            if let Some(text) = clipboard.text() {
                self.insert_text(text.as_ref(), cx);
            }
        }
    }

    pub fn apply_style(&mut self, style: RichTextStyle, cx: &mut Context<Self>) {
        let (start, end) = self.selection.normalized();
        if start == end {
            return;
        }

        // Check if the style already exists for this range
        let has_style = self
            .spans
            .iter()
            .any(|s| s.style == style && s.contains(start, end));

        if has_style {
            // Remove the style
            self.remove_style(start, end, &style);
        } else {
            // Add the style
            self.spans.push(TextSpan::new(start, end, style));
            self.merge_spans();
        }

        self.push_history();
        cx.notify();
    }

    fn remove_style(&mut self, start: usize, end: usize, style: &RichTextStyle) {
        let mut new_spans: Vec<TextSpan> = Vec::new();

        for span in self.spans.drain(..) {
            if span.style != *style {
                new_spans.push(span);
                continue;
            }

            // Split or remove the span based on overlap
            if span.end <= start || span.start >= end {
                // No overlap
                new_spans.push(span);
            } else if span.start >= start && span.end <= end {
                // Span is completely within removal range, remove it
            } else if span.start < start && span.end > end {
                // Removal range is within span, split it
                new_spans.push(TextSpan::new(span.start, start, style.clone()));
                new_spans.push(TextSpan::new(end, span.end, style.clone()));
            } else if span.start < start {
                // Span overlaps start of removal range
                new_spans.push(TextSpan::new(span.start, start, style.clone()));
            } else {
                // Span overlaps end of removal range
                new_spans.push(TextSpan::new(end, span.end, style.clone()));
            }
        }

        self.spans = new_spans;
    }

    fn merge_spans(&mut self) {
        // Sort spans by start position and style
        self.spans.sort_by(|a, b| {
            a.start
                .cmp(&b.start)
                .then_with(|| format!("{:?}", a.style).cmp(&format!("{:?}", b.style)))
        });

        // Merge overlapping spans of the same style
        let mut merged: Vec<TextSpan> = Vec::new();
        for span in self.spans.drain(..) {
            if let Some(last) = merged.last_mut() {
                if last.style == span.style && last.end >= span.start {
                    last.end = last.end.max(span.end);
                    continue;
                }
            }
            merged.push(span);
        }
        self.spans = merged;
    }

    pub fn build_highlights(&self, cx: &App) -> Vec<(Range<usize>, HighlightStyle)> {
        let theme = cx.theme();

        // Convert spans to highlights
        let span_highlights: Vec<(Range<usize>, HighlightStyle)> = self
            .spans
            .iter()
            .map(|span| {
                let highlight = match &span.style {
                    RichTextStyle::Bold => HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        ..Default::default()
                    },
                    RichTextStyle::Italic => HighlightStyle {
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    },
                    RichTextStyle::Underline => HighlightStyle {
                        underline: Some(UnderlineStyle {
                            thickness: px(1.0),
                            color: None,
                            wavy: false,
                        }),
                        ..Default::default()
                    },
                    RichTextStyle::Strikethrough => HighlightStyle {
                        strikethrough: Some(StrikethroughStyle {
                            thickness: px(1.0),
                            color: None,
                        }),
                        ..Default::default()
                    },
                    RichTextStyle::Code => HighlightStyle {
                        background_color: Some(theme.muted),
                        color: Some(theme.accent_foreground),
                        ..Default::default()
                    },
                };
                (span.start..span.end, highlight)
            })
            .collect();

        // Merge overlapping highlights
        self.merge_overlapping_highlights(span_highlights)
    }

    /// Merge overlapping highlights by combining their styles
    fn merge_overlapping_highlights(
        &self,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        if highlights.is_empty() {
            return highlights;
        }

        // Collect all unique boundary points
        let mut boundaries: Vec<usize> = Vec::new();
        for (range, _) in &highlights {
            boundaries.push(range.start);
            boundaries.push(range.end);
        }
        boundaries.sort();
        boundaries.dedup();

        // For each segment between boundaries, combine all applicable styles
        let mut result: Vec<(Range<usize>, HighlightStyle)> = Vec::new();

        for i in 0..boundaries.len().saturating_sub(1) {
            let start = boundaries[i];
            let end = boundaries[i + 1];

            if start >= end {
                continue;
            }

            // Find all highlights that cover this segment
            let mut combined_style = HighlightStyle::default();

            for (range, style) in &highlights {
                if range.start <= start && range.end >= end {
                    // Merge the style
                    if style.font_weight.is_some() {
                        combined_style.font_weight = style.font_weight;
                    }
                    if style.font_style.is_some() {
                        combined_style.font_style = style.font_style;
                    }
                    if style.underline.is_some() {
                        combined_style.underline = style.underline.clone();
                    }
                    if style.strikethrough.is_some() {
                        combined_style.strikethrough = style.strikethrough.clone();
                    }
                    if style.background_color.is_some() {
                        combined_style.background_color = style.background_color;
                    }
                    if style.color.is_some() {
                        combined_style.color = style.color;
                    }
                }
            }

            // Only add if there's at least one style applied
            if combined_style.font_weight.is_some()
                || combined_style.font_style.is_some()
                || combined_style.underline.is_some()
                || combined_style.strikethrough.is_some()
                || combined_style.background_color.is_some()
                || combined_style.color.is_some()
            {
                result.push((start..end, combined_style));
            }
        }

        result
    }

    /// Get code spans for special rendering
    pub fn code_spans(&self) -> Vec<Range<usize>> {
        self.spans
            .iter()
            .filter(|s| s.style == RichTextStyle::Code)
            .map(|s| s.start..s.end)
            .collect()
    }

    fn handle_key_down(
        &mut self,
        _event: &KeyDownEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // Character input is now handled by EntityInputHandler via replace_text_in_range
        // This method is kept for potential future use with special key handling
    }
}

impl Focusable for RichTextState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl RichTextState {
    // UTF-16 conversion helpers for InputHandler
    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        let start = self.content[..range.start].encode_utf16().count();
        let end = self.content[..range.end].encode_utf16().count();
        start..end
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        let mut utf8_start = 0;
        let mut utf16_count = 0;

        for (i, c) in self.content.char_indices() {
            if utf16_count >= range_utf16.start {
                utf8_start = i;
                break;
            }
            utf16_count += c.len_utf16();
            if utf16_count >= range_utf16.start {
                utf8_start = i + c.len_utf8();
                break;
            }
        }
        if utf16_count < range_utf16.start {
            utf8_start = self.content.len();
        }

        let mut utf8_end = utf8_start;
        for (i, c) in self.content[utf8_start..].char_indices() {
            if utf16_count >= range_utf16.end {
                utf8_end = utf8_start + i;
                break;
            }
            utf16_count += c.len_utf16();
            if utf16_count >= range_utf16.end {
                utf8_end = utf8_start + i + c.len_utf8();
                break;
            }
        }
        if utf16_count < range_utf16.end {
            utf8_end = self.content.len();
        }

        utf8_start..utf8_end
    }
}

impl EntityInputHandler for RichTextState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let (start, end) = self.selection.normalized();
        let range = start..end;
        Some(UTF16Selection {
            range: self.range_to_utf16(&range),
            reversed: self.selection.start > self.selection.end,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (sel_start, sel_end) = self.selection.normalized();
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(sel_start..sel_end);

        self.content =
            self.content[..range.start].to_owned() + new_text + &self.content[range.end..];
        let new_cursor = range.start + new_text.len();
        self.selection = Selection::cursor(new_cursor);
        self.marked_range = None;
        cx.emit(RichTextEvent::Change(self.content.clone().into()));
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (sel_start, sel_end) = self.selection.normalized();
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(sel_start..sel_end);

        self.content =
            self.content[..range.start].to_owned() + new_text + &self.content[range.end..];

        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }

        let new_cursor = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| {
                let pos = range.start + new_text.len();
                pos..pos
            });
        self.selection = Selection::new(new_cursor.start, new_cursor.end);

        cx.emit(RichTextEvent::Change(self.content.clone().into()));
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // Return the bounds of the text area for IME positioning
        Some(bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

/// Wrapper component that provides full rendering with context menu support
#[derive(IntoElement)]
pub struct RichTextView {
    state: Entity<RichTextState>,
    style: StyleRefinement,
}

impl RichTextView {
    pub fn new(state: Entity<RichTextState>) -> Self {
        Self {
            state,
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for RichTextView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for RichTextView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let content = self.state.read(cx).content.clone();
        let highlights = self.state.read(cx).build_highlights(cx);
        let selection = self.state.read(cx).selection;
        let focus_handle = self.state.read(cx).focus_handle.clone();
        let is_focused = focus_handle.is_focused(window);
        let cursor_visible = self.state.read(cx).cursor_visible();

        let text_style = window.text_style();
        let theme = cx.theme();
        let font_size = text_style.font_size.to_pixels(window.rem_size());

        // Cursor position for IME/input handling
        let cursor_pos = selection.head().min(content.len());
        let line_height = font_size * 1.5;

        let state = self.state.clone();
        let style = self.style;

        let base = div()
            .id("rich-text-container")
            .key_context(CONTEXT)
            .track_focus(&focus_handle)
            .map(|mut this| {
                *this.style() = this.style().clone().refined(style);
                this
            })
            .on_key_down({
                let state = state.clone();
                move |event: &KeyDownEvent, window, cx| {
                    state.update(cx, |s, cx| {
                        s.handle_key_down(event, window, cx);
                    });
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Backspace, _, cx| {
                    state.update(cx, |s, cx| s.backspace(cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Delete, _, cx| {
                    state.update(cx, |s, cx| s.delete(cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Enter, _, cx| {
                    state.update(cx, |_s, cx| cx.emit(RichTextEvent::Enter));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Tab, _, cx| {
                    state.update(cx, |_s, cx| cx.emit(RichTextEvent::Tab));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Space, _, cx| {
                    state.update(cx, |s, cx| {
                        s.insert_text(" ", cx);
                        cx.emit(RichTextEvent::Space);
                    });
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Slash, _, cx| {
                    state.update(cx, |s, cx| {
                        s.insert_text("/", cx);
                        cx.emit(RichTextEvent::Slash);
                    });
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &MoveLeft, _, cx| {
                    state.update(cx, |s, cx| s.move_left(false, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &MoveRight, _, cx| {
                    state.update(cx, |s, cx| s.move_right(false, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &MoveToStart, _, cx| {
                    state.update(cx, |s, cx| s.move_to_start(cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &MoveToEnd, _, cx| {
                    state.update(cx, |s, cx| s.move_to_end(cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &SelectLeft, _, cx| {
                    state.update(cx, |s, cx| s.move_left(true, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &SelectRight, _, cx| {
                    state.update(cx, |s, cx| s.move_right(true, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &MoveWordLeft, _, cx| {
                    state.update(cx, |s, cx| s.move_word_left(false, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &MoveWordRight, _, cx| {
                    state.update(cx, |s, cx| s.move_word_right(false, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &SelectWordLeft, _, cx| {
                    state.update(cx, |s, cx| s.move_word_left(true, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &SelectWordRight, _, cx| {
                    state.update(cx, |s, cx| s.move_word_right(true, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &SelectAll, _, cx| {
                    state.update(cx, |s, cx| s.select_all(cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Copy, _, cx| {
                    state.update(cx, |s, cx| s.copy(cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Cut, _, cx| {
                    state.update(cx, |s, cx| s.cut(cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Paste, _, cx| {
                    state.update(cx, |s, cx| s.paste(cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Undo, _, cx| {
                    state.update(cx, |s, cx| s.undo(cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &Redo, _, cx| {
                    state.update(cx, |s, cx| s.redo(cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &ToggleBold, _, cx| {
                    state.update(cx, |s, cx| s.apply_style(RichTextStyle::Bold, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &ToggleItalic, _, cx| {
                    state.update(cx, |s, cx| s.apply_style(RichTextStyle::Italic, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &ToggleUnderline, _, cx| {
                    state.update(cx, |s, cx| s.apply_style(RichTextStyle::Underline, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &ToggleStrikethrough, _, cx| {
                    state.update(cx, |s, cx| s.apply_style(RichTextStyle::Strikethrough, cx));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &ToggleCode, _, cx| {
                    state.update(cx, |s, cx| s.apply_style(RichTextStyle::Code, cx));
                }
            })
            .on_action({
                move |_: &ShowCharacterPalette, window, _cx| {
                    window.show_character_palette();
                }
            })
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                let focus_handle = focus_handle.clone();
                move |event: &MouseDownEvent, window, cx| {
                    focus_handle.focus(window);
                    state.update(cx, |s, cx| {
                        s.handle_mouse_down(event.position, event.click_count, window, cx);
                    });
                }
            })
            .on_mouse_down(MouseButton::Right, {
                let focus_handle = focus_handle.clone();
                move |_: &MouseDownEvent, window, _cx| {
                    // Just focus, don't change selection on right-click
                    focus_handle.focus(window);
                }
            })
            .on_mouse_move({
                let state = state.clone();
                move |event: &MouseMoveEvent, window, cx| {
                    if event.dragging() {
                        state.update(cx, |s, cx| {
                            s.handle_mouse_move(event.position, window, cx);
                        });
                    }
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let state = state.clone();
                move |_: &MouseUpEvent, window, cx| {
                    state.update(cx, |s, cx| {
                        s.handle_mouse_up(window, cx);
                    });
                }
            })
            .w_full()
            .cursor_text()
            .min_h(line_height)
            .relative()
            // Selection and cursor overlay (absolute positioned, painted first as background)
            .child({
                let state_for_bounds = state.clone();
                let state_for_overlay = state.clone();
                let selection_for_overlay = selection;
                let cursor_pos_for_overlay = cursor_pos;
                let is_focused_for_overlay = is_focused;
                let cursor_visible_for_overlay = cursor_visible;
                let theme_selection = theme.selection;
                let theme_foreground = theme.foreground;

                canvas(
                    move |bounds, _window, cx| {
                        state_for_bounds.update(cx, |s, cx| {
                            s.set_bounds(bounds, cx);
                        });
                    },
                    move |bounds, _, window, cx| {
                        let text_style = window.text_style();
                        let font_size = text_style.font_size.to_pixels(window.rem_size());
                        let line_height = font_size * 1.5;

                        // Paint selection and cursor using shape_text for accurate positioning
                        let wrap_width = bounds.size.width;
                        let content = state_for_overlay.read(cx).content().to_string();
                        let display_content = if content.is_empty() {
                            " ".to_string()
                        } else {
                            content
                        };

                        // Use shape_text with wrap to get accurate positions
                        let wrapped = window.text_system().shape_text(
                            SharedString::from(display_content.clone()),
                            font_size,
                            &[text_style.to_run(display_content.len())],
                            Some(wrap_width),
                            None,
                        );

                        if let Ok(wrapped) = wrapped {
                            // Paint selection
                            if is_focused_for_overlay && !selection_for_overlay.is_empty() {
                                let (sel_start, sel_end) = selection_for_overlay.normalized();

                                for line in wrapped.iter() {
                                    if let Some(start_pos) =
                                        line.position_for_index(sel_start, line_height)
                                    {
                                        if let Some(end_pos) =
                                            line.position_for_index(sel_end, line_height)
                                        {
                                            // Same line selection
                                            if (start_pos.y - end_pos.y).abs() < px(1.0) {
                                                let width = end_pos.x - start_pos.x;
                                                if width > px(0.0) {
                                                    let rect = gpui::Bounds::new(
                                                        gpui::point(
                                                            bounds.left() + start_pos.x,
                                                            bounds.top() + start_pos.y,
                                                        ),
                                                        gpui::size(width, line_height),
                                                    );
                                                    window.paint_quad(gpui::fill(
                                                        rect,
                                                        theme_selection,
                                                    ));
                                                }
                                            } else {
                                                // Multi-line selection
                                                let start_line =
                                                    (start_pos.y / line_height).floor() as i32;
                                                let end_line =
                                                    (end_pos.y / line_height).floor() as i32;

                                                for line_idx in start_line..=end_line {
                                                    let y = line_idx as f32 * line_height;
                                                    let (x_start, x_end) = if line_idx == start_line
                                                    {
                                                        (start_pos.x, wrap_width)
                                                    } else if line_idx == end_line {
                                                        (px(0.0), end_pos.x)
                                                    } else {
                                                        (px(0.0), wrap_width)
                                                    };

                                                    let width = x_end - x_start;
                                                    if width > px(0.0) {
                                                        let rect = gpui::Bounds::new(
                                                            gpui::point(
                                                                bounds.left() + x_start,
                                                                bounds.top() + y,
                                                            ),
                                                            gpui::size(width, line_height),
                                                        );
                                                        window.paint_quad(gpui::fill(
                                                            rect,
                                                            theme_selection,
                                                        ));
                                                    }
                                                }
                                            }
                                            break;
                                        }
                                    }
                                }
                            }

                            // Paint cursor
                            if is_focused_for_overlay
                                && cursor_visible_for_overlay
                                && selection_for_overlay.is_empty()
                            {
                                for line in wrapped.iter() {
                                    if let Some(cursor_pos) =
                                        line.position_for_index(cursor_pos_for_overlay, line_height)
                                    {
                                        let cursor_bounds = gpui::Bounds::new(
                                            gpui::point(
                                                bounds.left() + cursor_pos.x,
                                                bounds.top() + cursor_pos.y,
                                            ),
                                            gpui::size(px(2.0), line_height),
                                        );
                                        window.paint_quad(gpui::fill(
                                            cursor_bounds,
                                            theme_foreground,
                                        ));
                                        break;
                                    }
                                }
                            }
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full()
            })
            // Text content using StyledText (determines the height)
            .child({
                let state_for_input = state.clone();
                let display_content: SharedString = if content.is_empty() {
                    " ".into()
                } else {
                    content.clone().into()
                };

                div()
                    .id("rich-text-content")
                    .w_full()
                    .child(gpui::StyledText::new(display_content).with_highlights(highlights))
                    .child(canvas(
                        |_, _, _| {},
                        move |bounds, _, window, cx| {
                            // Register input handler
                            window.handle_input(
                                &state_for_input.read(cx).focus_handle,
                                ElementInputHandler::new(bounds, state_for_input.clone()),
                                cx,
                            );
                        },
                    ))
            });

        // Always add context menu - it will show styling options when there's a selection
        base.context_menu({
            let state = self.state.clone();
            move |menu, _window, cx| {
                let has_sel = !state.read(cx).selection.is_empty();
                if has_sel {
                    menu.menu("Bold", Box::new(ToggleBold))
                        .menu("Italic", Box::new(ToggleItalic))
                        .menu("Underline", Box::new(ToggleUnderline))
                        .menu("Strikethrough", Box::new(ToggleStrikethrough))
                        .menu("Code", Box::new(ToggleCode))
                } else {
                    menu
                }
            }
        })
        .into_any_element()
    }
}

impl EventEmitter<RichTextEvent> for RichTextView {}
