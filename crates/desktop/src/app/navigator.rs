use gpui_kit::{Context, Entity, Global};
use remindr_core::domain::{BlockId, DatabaseId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Page(BlockId),
    Database(DatabaseId),
}

#[derive(Default)]
pub struct Navigator {
    route: Option<Route>,
    back: Vec<Route>,
}

impl Navigator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, route: Route, cx: &mut Context<Self>) {
        if let Some(current) = self.route.replace(route) {
            self.back.push(current);
        }
        cx.notify();
    }

    pub fn back(&mut self, cx: &mut Context<Self>) {
        if let Some(previous) = self.back.pop() {
            self.route = Some(previous);
            cx.notify();
        }
    }

    #[must_use]
    pub fn route(&self) -> Option<&Route> {
        self.route.as_ref()
    }
}

pub struct NavigatorHandle(pub Entity<Navigator>);

impl Global for NavigatorHandle {}
