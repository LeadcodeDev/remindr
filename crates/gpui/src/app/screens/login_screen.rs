use gpui::{Context, IntoElement, ParentElement, Render, Window, div};

pub struct LoginScreen {}

impl LoginScreen {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for LoginScreen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child("Login Screen")
    }
}
