use gpui::*;
use gpui_component::ActiveTheme;
use gpui_router::{IntoLayout, Outlet, use_location};

use crate::components::sidebar::AppSidebar;

#[derive(IntoElement, IntoLayout)]
pub struct Layout {
    sidebar: AppSidebar,
    outlet: Outlet,
}

impl Layout {
    pub fn new() -> Self {
        Self {
            sidebar: AppSidebar,
            outlet: Outlet::new(),
        }
    }
}

impl RenderOnce for Layout {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let location = use_location(cx);
        if location.pathname != SharedString::from("/login") {}

        div()
            .w_full()
            .h_full()
            .flex()
            .child(div().bg(cx.theme().accent).child(self.sidebar))
            .child(self.outlet)
    }
}
