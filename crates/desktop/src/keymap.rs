use gpui_kit::{App, KeyBinding, actions};

actions!(shell, [Undo, Redo, NewPage, Back, Quit]);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-z", Undo, None),
        KeyBinding::new("cmd-shift-z", Redo, None),
        KeyBinding::new("cmd-n", NewPage, None),
        KeyBinding::new("cmd-[", Back, None),
        KeyBinding::new("cmd-alt-left", Back, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Keystroke, TestAppContext};

    use super::init;

    fn bound_to(cx: &mut TestAppContext, keystroke: &str) -> Vec<String> {
        cx.update(|cx| {
            let keystroke = Keystroke::parse(keystroke).unwrap();
            cx.all_bindings_for_input(&[keystroke])
                .iter()
                .map(|binding| binding.action().name().to_owned())
                .collect()
        })
    }

    #[gpui_kit::test]
    fn global_shortcuts_are_bound_without_a_context(cx: &mut TestAppContext) {
        cx.update(init);
        assert_eq!(bound_to(cx, "cmd-z"), vec!["shell::Undo"]);
        assert_eq!(bound_to(cx, "cmd-shift-z"), vec!["shell::Redo"]);
        assert_eq!(bound_to(cx, "cmd-n"), vec!["shell::NewPage"]);
        assert_eq!(bound_to(cx, "cmd-["), vec!["shell::Back"]);
        assert_eq!(bound_to(cx, "cmd-alt-left"), vec!["shell::Back"]);
        assert_eq!(bound_to(cx, "cmd-q"), vec!["shell::Quit"]);
    }
}
