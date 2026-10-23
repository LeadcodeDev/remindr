use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    App, ElementId, Entity, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::PageView;

pub const ICON_SUGGESTIONS: [&str; 24] = [
    "📄", "📝", "📌", "📚", "💡", "✅", "🚀", "🎯", "📅", "🗂️", "🧠", "🔖", "⭐", "🔥", "🌱", "🛠️",
    "📊", "💼", "🏠", "🎨", "🧪", "📦", "🔒", "❤️",
];

const HEADER_GROUP: &str = "page-header";

#[must_use]
pub fn icon_option_id(index: usize) -> ElementId {
    ElementId::NamedInteger("icon-option".into(), index as u64)
}

#[derive(IntoElement)]
pub struct Picker {
    pub view: Entity<PageView>,
    pub input: Entity<InputState>,
    pub open: bool,
    pub current: Option<String>,
}

impl RenderOnce for Picker {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let has_icon = self.current.is_some();
        let reveal_on_hover = !has_icon && !self.open;
        let trigger = match &self.current {
            Some(emoji) => Button::new("page-icon")
                .ghost()
                .h(px(72.))
                .px_2()
                .accessibility_label(emoji.clone())
                .child(div().text_size(px(48.)).child(emoji.clone())),
            None => Button::new("page-icon")
                .ghost()
                .small()
                .label("Add icon")
                .accessibility_label("Add icon"),
        };
        let open_view = self.view.clone();
        let content_view = self.view;
        let content_input = self.input;
        h_flex()
            .when(reveal_on_hover, |this| {
                this.opacity(0.)
                    .group_hover(HEADER_GROUP, |style| style.opacity(1.))
            })
            .child(
                Popover::new("page-icon-popover")
                    .open(self.open)
                    .on_open_change(move |open, _, cx| {
                        let open = *open;
                        open_view.update(cx, |this, cx| {
                            this.icon_open = open;
                            cx.notify();
                        });
                    })
                    .trigger(trigger)
                    .content(move |_, _, _| {
                        content(content_view.clone(), content_input.clone(), has_icon)
                    }),
            )
    }
}

fn content(view: Entity<PageView>, input: Entity<InputState>, has_icon: bool) -> impl IntoElement {
    let options = ICON_SUGGESTIONS.iter().enumerate().map(|(index, emoji)| {
        let view = view.clone();
        Button::new(icon_option_id(index))
            .ghost()
            .accessibility_label(*emoji)
            .label(*emoji)
            .on_click(move |_, window, cx| {
                view.update(cx, |this, cx| this.choose_icon(Some(emoji), window, cx));
            })
    });
    let field = Input::new(&input).id("icon-input");
    v_flex()
        .w(px(300.))
        .gap_2()
        .child(h_flex().flex_wrap().gap_1().children(options))
        .child(field)
        .child(
            div().child(
                Button::new("icon-remove")
                    .ghost()
                    .small()
                    .label("Remove")
                    .disabled(!has_icon)
                    .on_click(move |_, window, cx| {
                        view.update(cx, |this, cx| this.choose_icon(None, window, cx));
                    }),
            ),
        )
}
