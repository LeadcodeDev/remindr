use gpui::*;
use gpui_component::{ActiveTheme, Root};

use crate::app::{
    components::sidebar::AppSidebar, screens::home_screen::HomeScreen, states::app_state::AppState,
};

pub mod document_screen;
pub mod home_screen;
pub mod login_screen;

pub struct AppRouter {
    app_state: Entity<AppState>,
    sidebar: Entity<AppSidebar>,
}

impl AppRouter {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let app_state = cx.new(|cx| {
            let mut state = AppState::new();
            let home = HomeScreen::new(cx.weak_entity());
            state.navigator.push(home, cx);
            state
        });

        Self {
            app_state: app_state.clone(),
            sidebar: AppSidebar::new(app_state, cx),
        }
    }
}

impl Render for AppRouter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .w_full()
            .h_full()
            .flex()
            .child(div().bg(cx.theme().accent).child(self.sidebar.clone()))
            .child(
                if let Some(current_view) = self.app_state.read(cx).navigator.current() {
                    current_view.clone()
                } else {
                    AnyView::from(cx.new(|_| EmptyView))
                },
            )
            .children(notification_layer)
    }
}
