use gpui::*;

pub struct HomeScreen {}

impl HomeScreen {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for HomeScreen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child("Home Screen")
    }
}
