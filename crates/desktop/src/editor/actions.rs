use gpui_kit::{Action, App, Context, InteractiveElement, KeyBinding, Window, actions};

use super::format::Mark;
use super::{BlockEditor, KEY_CONTEXT};

actions!(
    remindr_editor,
    [
        Backspace,
        Delete,
        Enter,
        SoftBreak,
        InsertBelow,
        Indent,
        Outdent,
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        MoveHome,
        MoveEnd,
        MoveWordLeft,
        MoveWordRight,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectHome,
        SelectEnd,
        SelectWordLeft,
        SelectWordRight,
        SelectAll,
        Undo,
        Redo,
        Copy,
        Cut,
        Paste,
        Escape,
        DeleteWordBackward,
        DeleteWordForward,
        DeleteLineBackward,
        ToggleBold,
        ToggleItalic,
        ToggleUnderline,
        ToggleStrikethrough,
        ToggleCode,
        EditLink,
    ]
);

pub fn bind(cx: &mut App) {
    let context = Some(KEY_CONTEXT);
    let word = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    let mut bindings = vec![
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("enter", Enter, context),
        KeyBinding::new("escape", Escape, context),
        KeyBinding::new("shift-enter", SoftBreak, context),
        KeyBinding::new("secondary-enter", InsertBelow, context),
        KeyBinding::new("tab", Indent, context),
        KeyBinding::new("shift-tab", Outdent, context),
        KeyBinding::new("left", MoveLeft, context),
        KeyBinding::new("right", MoveRight, context),
        KeyBinding::new("up", MoveUp, context),
        KeyBinding::new("down", MoveDown, context),
        KeyBinding::new("home", MoveHome, context),
        KeyBinding::new("end", MoveEnd, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new("shift-home", SelectHome, context),
        KeyBinding::new("shift-end", SelectEnd, context),
        KeyBinding::new(&format!("{word}-left"), MoveWordLeft, context),
        KeyBinding::new(&format!("{word}-right"), MoveWordRight, context),
        KeyBinding::new(&format!("{word}-shift-left"), SelectWordLeft, context),
        KeyBinding::new(&format!("{word}-shift-right"), SelectWordRight, context),
        KeyBinding::new("secondary-a", SelectAll, context),
        KeyBinding::new("secondary-z", Undo, context),
        KeyBinding::new("secondary-shift-z", Redo, context),
        KeyBinding::new("secondary-c", Copy, context),
        KeyBinding::new("secondary-x", Cut, context),
        KeyBinding::new("secondary-v", Paste, context),
        KeyBinding::new(&format!("{word}-backspace"), DeleteWordBackward, context),
        KeyBinding::new(&format!("{word}-delete"), DeleteWordForward, context),
        KeyBinding::new("secondary-b", ToggleBold, context),
        KeyBinding::new("secondary-i", ToggleItalic, context),
        KeyBinding::new("secondary-u", ToggleUnderline, context),
        KeyBinding::new("secondary-shift-s", ToggleStrikethrough, context),
        KeyBinding::new("secondary-e", ToggleCode, context),
        KeyBinding::new("secondary-k", EditLink, context),
    ];
    if cfg!(target_os = "macos") {
        bindings.extend([
            KeyBinding::new("cmd-left", MoveHome, context),
            KeyBinding::new("cmd-right", MoveEnd, context),
            KeyBinding::new("cmd-shift-left", SelectHome, context),
            KeyBinding::new("cmd-shift-right", SelectEnd, context),
            KeyBinding::new("cmd-backspace", DeleteLineBackward, context),
        ]);
    } else {
        bindings.push(KeyBinding::new("ctrl-y", Redo, context));
    }
    cx.bind_keys(bindings);
}

type Run = fn(&mut BlockEditor, &mut Window, &mut Context<BlockEditor>);

trait Acts: InteractiveElement + Sized {
    fn act<A: Action>(self, cx: &mut Context<BlockEditor>, run: Run) -> Self {
        self.on_action(cx.listener(move |this, _: &A, window, cx| {
            if this.modal() {
                return;
            }
            this.commit_marked(window, cx);
            run(this, window, cx);
        }))
    }

    fn format<A: Action>(self, cx: &mut Context<BlockEditor>, run: Run) -> Self {
        self.on_action(cx.listener(move |this, _: &A, window, cx| {
            if !this.focus.is_focused(window) {
                cx.propagate();
                return;
            }
            if this.modal() {
                return;
            }
            this.commit_marked(window, cx);
            run(this, window, cx);
        }))
    }
}

impl<E: InteractiveElement> Acts for E {}

pub fn handle<E: InteractiveElement>(root: E, cx: &mut Context<BlockEditor>) -> E {
    root.on_action(cx.listener(|this, _: &Backspace, window, cx| {
        this.commit_marked(window, cx);
        if this.modal_backspace(cx) {
            return;
        }
        this.backspace(window, cx);
    }))
    .act::<Delete>(cx, |this, window, cx| this.delete_forward(window, cx))
    .on_action(cx.listener(|this, _: &Enter, window, cx| {
        this.commit_marked(window, cx);
        if this.overlay_enter(window, cx) {
            return;
        }
        this.enter(window, cx);
    }))
    .on_action(cx.listener(|this, _: &Escape, _, cx| {
        if !this.dismiss_overlay(cx) {
            cx.propagate();
        }
    }))
    .act::<SoftBreak>(cx, |this, window, cx| this.type_text("\n", window, cx))
    .act::<InsertBelow>(cx, |this, window, cx| this.insert_below(window, cx))
    .act::<Indent>(cx, |this, window, cx| this.indent(window, cx))
    .act::<Outdent>(cx, |this, window, cx| this.outdent(window, cx))
    .act::<MoveLeft>(cx, |this, _, cx| {
        this.step_horizontal(false, false, false, cx)
    })
    .act::<MoveRight>(cx, |this, _, cx| {
        this.step_horizontal(true, false, false, cx)
    })
    .act::<SelectLeft>(cx, |this, _, cx| {
        this.step_horizontal(false, false, true, cx)
    })
    .act::<SelectRight>(cx, |this, _, cx| {
        this.step_horizontal(true, false, true, cx)
    })
    .act::<MoveWordLeft>(cx, |this, _, cx| {
        this.step_horizontal(false, true, false, cx)
    })
    .act::<MoveWordRight>(cx, |this, _, cx| {
        this.step_horizontal(true, true, false, cx)
    })
    .act::<SelectWordLeft>(cx, |this, _, cx| {
        this.step_horizontal(false, true, true, cx)
    })
    .act::<SelectWordRight>(cx, |this, _, cx| this.step_horizontal(true, true, true, cx))
    .on_action(cx.listener(|this, _: &MoveUp, window, cx| {
        this.commit_marked(window, cx);
        if this.overlay_step(false, cx) {
            return;
        }
        this.step_vertical(false, false, cx);
    }))
    .on_action(cx.listener(|this, _: &MoveDown, window, cx| {
        this.commit_marked(window, cx);
        if this.overlay_step(true, cx) {
            return;
        }
        this.step_vertical(true, false, cx);
    }))
    .act::<SelectUp>(cx, |this, _, cx| this.step_vertical(false, true, cx))
    .act::<SelectDown>(cx, |this, _, cx| this.step_vertical(true, true, cx))
    .act::<MoveHome>(cx, |this, _, cx| this.step_line_edge(false, false, cx))
    .act::<MoveEnd>(cx, |this, _, cx| this.step_line_edge(true, false, cx))
    .act::<SelectHome>(cx, |this, _, cx| this.step_line_edge(false, true, cx))
    .act::<SelectEnd>(cx, |this, _, cx| this.step_line_edge(true, true, cx))
    .act::<SelectAll>(cx, |this, _, cx| this.select_all(cx))
    .act::<Undo>(cx, |this, window, cx| this.undo(window, cx))
    .act::<Redo>(cx, |this, window, cx| this.redo(window, cx))
    .act::<DeleteWordBackward>(cx, |this, window, cx| this.delete_word_backward(window, cx))
    .act::<DeleteWordForward>(cx, |this, window, cx| this.delete_word_forward(window, cx))
    .act::<DeleteLineBackward>(cx, |this, window, cx| this.delete_line_backward(window, cx))
    .format::<ToggleBold>(cx, |this, window, cx| {
        this.toggle_mark(Mark::Bold, window, cx)
    })
    .format::<ToggleItalic>(cx, |this, window, cx| {
        this.toggle_mark(Mark::Italic, window, cx)
    })
    .format::<ToggleUnderline>(cx, |this, window, cx| {
        this.toggle_mark(Mark::Underline, window, cx)
    })
    .format::<ToggleStrikethrough>(cx, |this, window, cx| {
        this.toggle_mark(Mark::Strikethrough, window, cx)
    })
    .format::<ToggleCode>(cx, |this, window, cx| {
        this.toggle_mark(Mark::Code, window, cx)
    })
    .format::<EditLink>(cx, |this, _, cx| this.open_link(cx))
    .on_action(cx.listener(|this, _: &Copy, _, cx| {
        if !this.modal() {
            this.copy(cx);
        }
    }))
    .on_action(cx.listener(|this, _: &Cut, window, cx| {
        if !this.modal() {
            this.cut(window, cx);
        }
    }))
    .on_action(cx.listener(|this, _: &Paste, window, cx| this.paste(window, cx)))
}
