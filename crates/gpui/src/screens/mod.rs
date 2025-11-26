use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_router::{Route, Routes};

use crate::{
    components::layout::Layout,
    screens::{
        document_screen::DocumentScreen, home_screen::HomeScreen, login_screen::LoginScreen,
    },
};

pub mod document_screen;
pub mod home_screen;
pub mod login_screen;

pub struct AppRouter {
    login_screen: Entity<LoginScreen>,
    home_screen: Entity<HomeScreen>,
    document_screen: Entity<DocumentScreen>,
}

impl AppRouter {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        let login_screen = cx.new(|cx| LoginScreen::new(cx));
        let home_screen = cx.new(|cx| HomeScreen::new(cx));
        let document_screen = cx.new(|cx| DocumentScreen::new(cx));

        Self {
            login_screen,
            home_screen,
            document_screen,
        }
    }
}

impl Render for AppRouter {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w_full().h_full().child(
            Routes::new()
                .child(
                    Route::new()
                        .path("login")
                        .element(self.login_screen.clone()),
                )
                .child(
                    Route::new()
                        .layout(Layout::new())
                        .child(Route::new().index().element(self.home_screen.clone()))
                        .child(
                            Route::new()
                                .path("documents")
                                .element(self.document_screen.clone()),
                        )
                        .child(
                            Route::new()
                                .path("{*not_match}")
                                .element(div().child("Not found")),
                        ),
                ),
        )
    }
}
