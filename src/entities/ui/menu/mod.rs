use gpui::{
    AppContext, Context, Corner, Element, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, Window, div, px,
};
use gpui_component::button::Button;
use gpui_component::popover::{Popover, PopoverContent};
use gpui_component::{ActiveTheme, divider::Divider, h_flex};
use gpui_component::{IconName, IndexPath, StyledExt};

use gpui_component::dropdown::{
    Dropdown, DropdownDelegate, DropdownEvent, DropdownItem, DropdownItemGroup, DropdownState,
    SearchableVec,
};

pub struct Menu {
    dropdown: Entity<DropdownState<SearchableVec<String>>>,
}

impl Menu {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let fruits = SearchableVec::new(vec![
            "Apple".into(),
            "Orange".into(),
            "Banana".into(),
            "Grape".into(),
            "Pineapple".into(),
            "Watermelon & This is a long long long long long long long long long title".into(),
            "Avocado".into(),
        ]);

        let fruit_dropdown = cx.new(|cx| DropdownState::new(fruits, None, window, cx));

        cx.subscribe_in(
            &fruit_dropdown,
            window,
            |_, _, _: &DropdownEvent<SearchableVec<String>>, _, _| {
                println!("Event !");
            },
        )
        .detach();

        Self {
            dropdown: fruit_dropdown,
        }
    }
}

impl Render for Menu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().max_w_128().child(
            Dropdown::new(&self.dropdown)
                .icon(IconName::Search)
                .w(px(320.))
                .menu_width(px(400.))
                .placeholder("Hello World")
                .absolute(),
        )
    }
}
