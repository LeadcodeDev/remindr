use std::ops::DerefMut;

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
};

use crate::app::states::settings_state::{Settings, ThemeMode};

pub struct TitleBar;

impl TitleBar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }

    fn cycle_theme_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.update_global::<Settings, _>(|settings, _cx| {
            settings.theme.mode = settings.theme.mode.next();
        });

        // Apply the new theme
        crate::app::apply_theme(window, cx.deref_mut());
    }
}

impl Render for TitleBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme_mode = cx
            .try_global::<Settings>()
            .map(|s| s.theme.mode)
            .unwrap_or_default();

        let (icon, tooltip_text) = match theme_mode {
            ThemeMode::Light => (Icon::new(IconName::Sun), "Light mode"),
            ThemeMode::Dark => (Icon::new(IconName::Moon), "Dark mode"),
            ThemeMode::System => (
                Icon::new(IconName::Sun).path("icons/sun-moon.svg"),
                "System mode",
            ),
        };

        div()
            .id("title-bar")
            .w_full()
            .h(px(36.))
            .bg(cx.theme().title_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .flex()
            .items_center()
            .justify_end()
            .px_3()
            .child(
                Button::new("theme-toggle")
                    .icon(icon)
                    .ghost()
                    .small()
                    .tooltip(tooltip_text)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.cycle_theme_mode(window, cx);
                    })),
            )
    }
}
