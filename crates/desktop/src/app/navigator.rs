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
        if self.route == Some(route) {
            return;
        }
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

    pub fn prune(&mut self, alive: impl Fn(&Route) -> bool) -> bool {
        let history = self.back.len();
        self.back.retain(&alive);
        let mut changed = self.back.len() != history;
        if self.route.as_ref().is_some_and(|route| !alive(route)) {
            self.route = self.back.pop();
            changed = true;
        }
        changed
    }
}

pub struct NavigatorHandle(pub Entity<Navigator>);

impl Global for NavigatorHandle {}

#[cfg(test)]
mod tests {
    use gpui_kit::{AppContext, TestAppContext};
    use remindr_core::domain::BlockId;

    use super::{Navigator, Route};

    fn pages() -> (Route, Route, Route) {
        (
            Route::Page(BlockId::new()),
            Route::Page(BlockId::new()),
            Route::Page(BlockId::new()),
        )
    }

    #[gpui_kit::test]
    fn opening_the_current_route_keeps_a_single_history_entry(cx: &mut TestAppContext) {
        let (first, second, _) = pages();
        let navigator = cx.new(|_| Navigator::new());
        navigator.update(cx, |navigator, cx| {
            navigator.open(first, cx);
            navigator.open(second, cx);
            navigator.open(second, cx);
            navigator.back(cx);
            assert_eq!(navigator.route(), Some(&first));
            navigator.back(cx);
            assert_eq!(navigator.route(), Some(&first));
        });
    }

    #[gpui_kit::test]
    fn pruning_a_dead_current_route_falls_back_to_the_latest_alive_entry(cx: &mut TestAppContext) {
        let (first, second, third) = pages();
        let navigator = cx.new(|_| Navigator::new());
        navigator.update(cx, |navigator, cx| {
            navigator.open(first, cx);
            navigator.open(second, cx);
            navigator.open(third, cx);
            let changed = navigator.prune(|route| *route != third && *route != second);
            assert!(changed);
            assert_eq!(navigator.route(), Some(&first));
            navigator.back(cx);
            assert_eq!(navigator.route(), Some(&first));
        });
    }

    #[gpui_kit::test]
    fn pruning_everything_clears_the_route(cx: &mut TestAppContext) {
        let (first, second, _) = pages();
        let navigator = cx.new(|_| Navigator::new());
        navigator.update(cx, |navigator, cx| {
            navigator.open(first, cx);
            navigator.open(second, cx);
            assert!(navigator.prune(|_| false));
            assert_eq!(navigator.route(), None);
            assert!(!navigator.prune(|_| false));
        });
    }

    #[gpui_kit::test]
    fn pruning_alive_routes_changes_nothing(cx: &mut TestAppContext) {
        let (first, second, _) = pages();
        let navigator = cx.new(|_| Navigator::new());
        navigator.update(cx, |navigator, cx| {
            navigator.open(first, cx);
            navigator.open(second, cx);
            assert!(!navigator.prune(|_| true));
            assert_eq!(navigator.route(), Some(&second));
        });
    }
}
