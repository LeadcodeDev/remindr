use gpui::*;
use gpui_component::popover::Popover;

pub struct SlashMenu {
    open: bool,
    pub search: Option<SharedString>,
}

impl SlashMenu {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            open: false,
            search: None,
        }
    }
}

impl Render for SlashMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Popover::new("controlled-popover")
            .open(self.open)
            .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                this.open = *open;
                cx.notify();
            }))
            .child("This popover's open state is controlled programmatically.")
    }
}
