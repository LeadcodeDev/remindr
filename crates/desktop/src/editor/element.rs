use std::collections::HashMap;
use std::ops::Range;

use gpui_kit::base::text::editor_host::collect_leaf_geometry;
use gpui_kit::{
    AnyElement, App, Bounds, DispatchPhase, Element, ElementId, ElementInputHandler, Entity,
    GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Window, div, fill, point,
    px, size,
};

use super::BlockEditor;
use super::cull::Class;
use super::frame::{CARET_WIDTH, Frame, leaf_key};
use super::palette::Palette;

pub struct EditorRoot {
    pub editor: Entity<BlockEditor>,
    pub child: Option<AnyElement>,
    pub decorations: HashMap<u64, Vec<(Range<usize>, Hsla)>>,
    pub palette: Palette,
}

pub struct Prepaint {
    hitbox: Hitbox,
}

impl IntoElement for EditorRoot {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditorRoot {
    type RequestLayoutState = AnyElement;
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui_kit::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut child = self
            .child
            .take()
            .unwrap_or_else(|| div().into_any_element());
        let layout = child.request_layout(window, cx);
        (layout, child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let decorations = std::mem::take(&mut self.decorations);
        let (_, mut geometries) = collect_leaf_geometry(decorations, || child.prepaint(window, cx));
        let state = self.editor.read(cx);
        let leaves = state
            .order
            .iter()
            .filter_map(|block| {
                geometries
                    .remove(&leaf_key(*block))
                    .map(|geometry| (*block, geometry))
            })
            .collect();
        let rows = state.row_bounds.take();
        let workspace = state.model.read(cx).workspace();
        let changed = state.heights.borrow_mut().observe(&rows, |block| {
            workspace.block(block).map(|found| Class::of(&found.kind))
        });
        state.layout_changed.set(changed);
        let frame = Frame::new(leaves, rows);
        *state.frame.borrow_mut() = frame;
        state.reveal_caret(window);
        state.settle_culling(window);
        state.refresh_drop(cx);
        if state.auto_scroll(window) {
            self.follow_pointer(window, cx);
        }
        Prepaint {
            hitbox: window.insert_hitbox(bounds, HitboxBehavior::Normal),
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.editor.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        self.listen(prepaint.hitbox.clone(), window);
        child.paint(window, cx);
        self.paint_composition(window, cx);
        self.paint_caret(window, cx);
        self.paint_drop(window, cx);
        if let Some(report) = self.editor.read(cx).first_paint.finish() {
            eprintln!("{report}");
        }
    }
}

impl EditorRoot {
    fn listen(&self, hitbox: Hitbox, window: &mut Window) {
        let editor = self.editor.clone();
        let area = hitbox.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble
                || event.button != MouseButton::Left
                || !area.is_hovered(window)
            {
                return;
            }
            editor.update(cx, |this, cx| this.mouse_down(event, window, cx));
        });
        let editor = self.editor.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            editor.update(cx, |this, cx| {
                this.pointer.set(Some(event.position));
                this.mouse_drag(event.position, window, cx)
            });
        });
        let editor = self.editor.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble {
                editor.update(cx, |this, _| {
                    this.dragging = false;
                    if this.drag_block.get().is_none() {
                        this.pointer.set(None);
                    }
                });
            }
        });
    }

    fn follow_pointer(&self, window: &mut Window, cx: &mut App) {
        let editor = self.editor.clone();
        window.defer(cx, move |window, cx| {
            editor.update(cx, |this, cx| {
                if let Some(pointer) = this.pointer.get().filter(|_| this.dragging) {
                    this.mouse_drag(pointer, window, cx);
                }
            });
        });
    }

    fn paint_drop(&self, window: &mut Window, cx: &mut App) {
        let Some(mark) = self.editor.read(cx).drop.get() else {
            return;
        };
        window.paint_quad(fill(
            Bounds::new(
                point(mark.x, mark.y - px(1.)),
                size((mark.right - mark.x).max(px(0.)), px(2.)),
            ),
            self.palette.accent,
        ));
    }

    fn paint_composition(&self, window: &mut Window, cx: &mut App) {
        let state = self.editor.read(cx);
        let Some(marked) = &state.marked else {
            return;
        };
        let frame = state.frame.borrow();
        let Some(leaf) = frame.leaf(marked.block) else {
            return;
        };
        let range = marked.display_range();
        let start = leaf.position_for_index(range.start);
        let end = leaf.position_for_index(range.end);
        let top = start.y + leaf.line_height() - px(1.);
        let width = (end.x - start.x).max(px(2.));
        window.paint_quad(fill(
            Bounds::new(point(start.x, top), size(width, px(1.))),
            self.palette.foreground,
        ));
    }

    fn follow_focus(&self, window: &mut Window, cx: &mut App) {
        let state = self.editor.read(cx);
        let focused = state.focus.is_focused(window);
        let active = state.blink.read(cx).is_active();
        if focused == active {
            return;
        }
        let blink = state.blink.clone();
        window.defer(cx, move |_, cx| {
            blink.update(cx, |blink, cx| {
                if focused {
                    blink.start(cx);
                } else {
                    blink.stop(cx);
                }
            });
        });
    }

    fn paint_caret(&self, window: &mut Window, cx: &mut App) {
        self.follow_focus(window, cx);
        let state = self.editor.read(cx);
        let target = match (&state.marked, state.selection) {
            (Some(marked), _) => Some((marked.block, marked.display_cursor().end)),
            (None, Some(selection)) if selection.is_collapsed() => {
                Some((selection.head.block, selection.head.offset))
            }
            _ => None,
        };
        let Some((block, offset)) = target else {
            return;
        };
        if !state.focus.is_focused(window) || !state.blink.read(cx).visible() {
            return;
        }
        let frame = state.frame.borrow();
        if let Some(caret) = frame.caret_bounds(block, offset, CARET_WIDTH) {
            window.paint_quad(fill(caret, self.palette.caret));
        }
    }
}
