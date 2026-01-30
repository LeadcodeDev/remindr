use gpui::*;
use gpui_component::{ActiveTheme, Root};

use crate::app::{
    components::{sidebar::AppSidebar, title_bar::TitleBar},
    screens::home_screen::HomeScreen,
    states::{app_state::AppState, settings_state::Settings},
};

pub mod database_view_screen;
pub mod document_screen;
pub mod home_screen;
pub mod login_screen;

pub struct AppRouter {
    app_state: Entity<AppState>,
    sidebar: Entity<AppSidebar>,
    title_bar: Entity<TitleBar>,
}

impl AppRouter {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let app_state = cx.new(|cx| {
            let mut state = AppState::new();
            let home = HomeScreen::new(cx.weak_entity());
            state.navigator.push(home, cx);
            state
        });

        cx.observe_global::<Settings>(|_this, cx| {
            cx.notify();
        })
        .detach();

        Self {
            app_state: app_state.clone(),
            sidebar: AppSidebar::new(app_state, cx),
            title_bar: cx.new(TitleBar::new),
        }
    }
}

impl Render for AppRouter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notification_layer = Root::render_notification_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);

        let ui_font_size = cx
            .try_global::<Settings>()
            .map(|s| s.appearance.ui_font_size)
            .unwrap_or(14.0);

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .text_size(px(ui_font_size))
            .child(self.title_bar.clone())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .min_h_0()
                    .overflow_hidden()
                    .child(div().bg(cx.theme().accent).child(self.sidebar.clone()))
                    .child(div().flex_1().min_w_0().overflow_hidden().child(
                        if let Some(current_view) = self.app_state.read(cx).navigator.current() {
                            current_view.clone()
                        } else {
                            AnyView::from(cx.new(|_| EmptyView))
                        },
                    )),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}
