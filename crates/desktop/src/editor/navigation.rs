use gpui_kit::{Context, MouseDownEvent, Pixels, Point, Window, point};
use remindr_core::application::TextPoint;

use super::BlockEditor;
use super::caret::{
    Selection, grapheme_after, grapheme_before, word_after, word_before, word_range,
};
use super::frame::{Frame, Hit};
use super::rows::{Row, extent, position};

impl BlockEditor {
    pub(super) fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        self.dismiss_overlay(cx);
        self.commit_marked(window, cx);
        if self.visible_rows(cx).is_empty() {
            self.first_block(window, cx);
            return;
        }
        let hit = self.frame.borrow().hit(event.position);
        let Some(hit) = hit else {
            return;
        };
        let at = hit.point();
        if event.modifiers.secondary()
            && let Hit::Text(text_point) = hit
            && self.open_link_under(text_point, event.position, cx)
        {
            return;
        }
        self.dragging = true;
        if event.modifiers.shift {
            let anchor = self.selection.map_or(at, |selection| selection.anchor);
            self.set_selection(Some(Selection { anchor, head: at }), cx);
            return;
        }
        match (hit, event.click_count) {
            (Hit::Text(point), 2) => self.select_word(point, cx),
            (Hit::Text(point), count) if count >= 3 => self.select_block_text(point.block, cx),
            _ => self.set_caret(at, cx),
        }
    }

    pub(super) fn mouse_drag(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dragging {
            return;
        }
        let hit = self.frame.borrow().hit(position);
        let (Some(hit), Some(selection)) = (hit, self.selection) else {
            return;
        };
        let head = hit.point();
        if head != selection.head {
            self.commit_marked(window, cx);
            self.set_selection(
                Some(Selection {
                    anchor: selection.anchor,
                    head,
                }),
                cx,
            );
        }
    }

    fn select_word(&mut self, at: TextPoint, cx: &mut Context<Self>) {
        let plain = self.plain_of(at.block, cx).unwrap_or_default();
        let range = word_range(&plain, at.offset);
        self.set_selection(
            Some(Selection {
                anchor: TextPoint {
                    block: at.block,
                    offset: range.start,
                },
                head: TextPoint {
                    block: at.block,
                    offset: range.end,
                },
            }),
            cx,
        );
    }

    fn select_block_text(&mut self, block: remindr_core::domain::BlockId, cx: &mut Context<Self>) {
        let length = self.text_len(block, cx);
        self.set_selection(
            Some(Selection {
                anchor: TextPoint { block, offset: 0 },
                head: TextPoint {
                    block,
                    offset: length,
                },
            }),
            cx,
        );
    }

    fn move_head(&mut self, target: TextPoint, extend: bool, cx: &mut Context<Self>) {
        let selection = match (extend, self.selection) {
            (true, Some(current)) => Selection {
                anchor: current.anchor,
                head: target,
            },
            _ => Selection::collapsed(target),
        };
        self.set_selection(Some(selection), cx);
    }

    pub(super) fn step_horizontal(
        &mut self,
        forward: bool,
        by_word: bool,
        extend: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(selection) = self.selection else {
            return;
        };
        let rows = self.visible_rows(cx);
        if !extend
            && !selection.is_collapsed()
            && let Some(span) = extent(&rows, selection.anchor, selection.head)
        {
            self.set_caret(if forward { span.end } else { span.start }, cx);
            return;
        }
        if let Some(target) = self.horizontal_target(selection.head, forward, by_word, &rows, cx) {
            self.move_head(target, extend, cx);
        }
    }

    fn horizontal_target(
        &self,
        head: TextPoint,
        forward: bool,
        by_word: bool,
        rows: &[Row],
        cx: &Context<Self>,
    ) -> Option<TextPoint> {
        let plain = self.plain_of(head.block, cx)?;
        let atomic = self
            .kind_of(head.block, cx)
            .is_none_or(|kind| super::is_atomic_row(&kind));
        let inside = if atomic {
            None
        } else {
            match (forward, by_word) {
                (false, false) => grapheme_before(&plain, head.offset),
                (true, false) => grapheme_after(&plain, head.offset),
                (false, true) => (head.offset > 0).then(|| word_before(&plain, head.offset)),
                (true, true) => {
                    (head.offset < plain.len()).then(|| word_after(&plain, head.offset))
                }
            }
        };
        if let Some(offset) = inside {
            return Some(TextPoint {
                block: head.block,
                offset,
            });
        }
        let index = position(rows, head.block)?;
        let neighbor = if forward {
            rows.get(index + 1)
        } else {
            index.checked_sub(1).and_then(|before| rows.get(before))
        }?;
        let offset = if forward {
            0
        } else {
            self.text_len(neighbor.block, cx)
        };
        Some(TextPoint {
            block: neighbor.block,
            offset,
        })
    }

    pub(super) fn step_vertical(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(selection) = self.selection else {
            return;
        };
        let rows = self.visible_rows(cx);
        let (goal, target) = {
            let frame = self.frame.borrow();
            let goal = self.goal_x.or_else(|| {
                frame.leaf(selection.head.block).map(|leaf| {
                    leaf.position_for_index(selection.head.offset.min(leaf.len()))
                        .x
                })
            });
            let target = vertical_target(&frame, &rows, selection.head, goal, down);
            (goal, target)
        };
        if let Some(target) = target {
            self.move_head(target, extend, cx);
            self.goal_x = goal;
        }
    }

    pub(super) fn step_line_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(selection) = self.selection else {
            return;
        };
        let head = selection.head;
        let plain = self.plain_of(head.block, cx).unwrap_or_default();
        let edges = self
            .frame
            .borrow()
            .line_edges(head.block, head.offset, &plain);
        let (start, finish) = edges.unwrap_or((0, plain.len()));
        let target = TextPoint {
            block: head.block,
            offset: if end { finish } else { start },
        };
        self.move_head(self.valid_point(target, cx), extend, cx);
    }

    pub(super) fn select_all(&mut self, cx: &mut Context<Self>) {
        let rows = self.visible_rows(cx);
        let (Some(first), Some(last)) = (rows.first(), rows.last()) else {
            return;
        };
        let end = self.text_len(last.block, cx);
        self.set_selection(
            Some(Selection {
                anchor: TextPoint {
                    block: first.block,
                    offset: 0,
                },
                head: TextPoint {
                    block: last.block,
                    offset: end,
                },
            }),
            cx,
        );
    }
}

fn vertical_target(
    frame: &Frame,
    rows: &[Row],
    head: TextPoint,
    goal: Option<Pixels>,
    down: bool,
) -> Option<TextPoint> {
    let leaf = frame.leaf(head.block);
    if let (Some(leaf), Some(goal)) = (leaf, goal) {
        let caret = leaf.position_for_index(head.offset.min(leaf.len()));
        let line = leaf.line_height();
        let probe = if down {
            caret.y + line * 1.5
        } else {
            caret.y - line * 0.5
        };
        let bounds = leaf.bounds();
        if probe >= bounds.top() && probe < bounds.bottom() {
            return Some(TextPoint {
                block: head.block,
                offset: leaf.index_for_position(point(goal, probe)).min(leaf.len()),
            });
        }
    }
    let index = position(rows, head.block)?;
    let neighbor = if down {
        rows.get(index + 1)
    } else {
        index.checked_sub(1).and_then(|before| rows.get(before))
    };
    match neighbor {
        Some(row) => Some(match frame.leaf(row.block) {
            Some(target) => {
                let bounds = target.bounds();
                let goal = goal.unwrap_or_else(|| bounds.left());
                let half = target.line_height() / 2.;
                let y = if down {
                    bounds.top() + half
                } else {
                    bounds.bottom() - half
                };
                TextPoint {
                    block: row.block,
                    offset: target.index_for_position(point(goal, y)).min(target.len()),
                }
            }
            None => TextPoint {
                block: row.block,
                offset: 0,
            },
        }),
        None => leaf.map(|leaf| TextPoint {
            block: head.block,
            offset: if down { leaf.len() } else { 0 },
        }),
    }
}
