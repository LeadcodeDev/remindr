use gpui_kit::{App, Context, InteractiveElement, KeyBinding, actions};

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
    ];
    if cfg!(target_os = "macos") {
        bindings.extend([
            KeyBinding::new("cmd-left", MoveHome, context),
            KeyBinding::new("cmd-right", MoveEnd, context),
            KeyBinding::new("cmd-shift-left", SelectHome, context),
            KeyBinding::new("cmd-shift-right", SelectEnd, context),
        ]);
    } else {
        bindings.push(KeyBinding::new("ctrl-y", Redo, context));
    }
    cx.bind_keys(bindings);
}

pub fn handle<E: InteractiveElement>(root: E, cx: &mut Context<BlockEditor>) -> E {
    root.on_action(cx.listener(|this, _: &Backspace, window, cx| {
        this.commit_marked(window, cx);
        this.backspace(window, cx);
    }))
    .on_action(cx.listener(|this, _: &Delete, window, cx| {
        this.commit_marked(window, cx);
        this.delete_forward(window, cx);
    }))
    .on_action(cx.listener(|this, _: &Enter, window, cx| {
        this.commit_marked(window, cx);
        this.enter(window, cx);
    }))
    .on_action(cx.listener(|this, _: &SoftBreak, window, cx| {
        this.commit_marked(window, cx);
        this.type_text("\n", window, cx);
    }))
    .on_action(cx.listener(|this, _: &InsertBelow, window, cx| {
        this.commit_marked(window, cx);
        this.insert_below(window, cx);
    }))
    .on_action(cx.listener(|this, _: &Indent, window, cx| {
        this.commit_marked(window, cx);
        this.indent(window, cx);
    }))
    .on_action(cx.listener(|this, _: &Outdent, window, cx| {
        this.commit_marked(window, cx);
        this.outdent(window, cx);
    }))
    .on_action(cx.listener(|this, _: &MoveLeft, window, cx| {
        this.commit_marked(window, cx);
        this.step_horizontal(false, false, false, cx);
    }))
    .on_action(cx.listener(|this, _: &MoveRight, window, cx| {
        this.commit_marked(window, cx);
        this.step_horizontal(true, false, false, cx);
    }))
    .on_action(cx.listener(|this, _: &SelectLeft, window, cx| {
        this.commit_marked(window, cx);
        this.step_horizontal(false, false, true, cx);
    }))
    .on_action(cx.listener(|this, _: &SelectRight, window, cx| {
        this.commit_marked(window, cx);
        this.step_horizontal(true, false, true, cx);
    }))
    .on_action(cx.listener(|this, _: &MoveWordLeft, window, cx| {
        this.commit_marked(window, cx);
        this.step_horizontal(false, true, false, cx);
    }))
    .on_action(cx.listener(|this, _: &MoveWordRight, window, cx| {
        this.commit_marked(window, cx);
        this.step_horizontal(true, true, false, cx);
    }))
    .on_action(cx.listener(|this, _: &SelectWordLeft, window, cx| {
        this.commit_marked(window, cx);
        this.step_horizontal(false, true, true, cx);
    }))
    .on_action(cx.listener(|this, _: &SelectWordRight, window, cx| {
        this.commit_marked(window, cx);
        this.step_horizontal(true, true, true, cx);
    }))
    .on_action(cx.listener(|this, _: &MoveUp, window, cx| {
        this.commit_marked(window, cx);
        this.step_vertical(false, false, cx);
    }))
    .on_action(cx.listener(|this, _: &MoveDown, window, cx| {
        this.commit_marked(window, cx);
        this.step_vertical(true, false, cx);
    }))
    .on_action(cx.listener(|this, _: &SelectUp, window, cx| {
        this.commit_marked(window, cx);
        this.step_vertical(false, true, cx);
    }))
    .on_action(cx.listener(|this, _: &SelectDown, window, cx| {
        this.commit_marked(window, cx);
        this.step_vertical(true, true, cx);
    }))
    .on_action(cx.listener(|this, _: &MoveHome, window, cx| {
        this.commit_marked(window, cx);
        this.step_line_edge(false, false, cx);
    }))
    .on_action(cx.listener(|this, _: &MoveEnd, window, cx| {
        this.commit_marked(window, cx);
        this.step_line_edge(true, false, cx);
    }))
    .on_action(cx.listener(|this, _: &SelectHome, window, cx| {
        this.commit_marked(window, cx);
        this.step_line_edge(false, true, cx);
    }))
    .on_action(cx.listener(|this, _: &SelectEnd, window, cx| {
        this.commit_marked(window, cx);
        this.step_line_edge(true, true, cx);
    }))
    .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
        this.commit_marked(window, cx);
        this.select_all(cx);
    }))
    .on_action(cx.listener(|this, _: &Undo, window, cx| {
        this.commit_marked(window, cx);
        this.undo(window, cx);
    }))
    .on_action(cx.listener(|this, _: &Redo, window, cx| {
        this.commit_marked(window, cx);
        this.redo(window, cx);
    }))
    .on_action(cx.listener(|this, _: &Copy, _, cx| this.copy(cx)))
    .on_action(cx.listener(|this, _: &Cut, window, cx| this.cut(window, cx)))
    .on_action(cx.listener(|this, _: &Paste, window, cx| this.paste(window, cx)))
}
