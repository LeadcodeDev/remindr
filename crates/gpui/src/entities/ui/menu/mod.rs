use gpui::prelude::FluentBuilder;
use gpui::{
    Context, EventEmitter, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, actions, div,
};
use gpui_component::ActiveTheme;

actions!(story, [MyAction]);

pub enum MenuEvent {
    Focus,
}

impl EventEmitter<MenuEvent> for Menu {}

pub struct Menu {
    pub search: Option<SharedString>,
    elements: Vec<String>,
    _focus_handle: FocusHandle,
}

impl Menu {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _focus_handle = cx.focus_handle().tab_stop(true);

        cx.on_focus(&_focus_handle, window, |_, _, cx| cx.emit(MenuEvent::Focus))
            .detach();

        Self {
            search: None,
            elements: vec!["Text".to_string(), "Codeblock".to_string()],
            _focus_handle,
        }
    }

    fn filter_elements(&self) -> Vec<&String> {
        self.elements
            .iter()
            .filter(|element| match self.search {
                Some(ref search) => element
                    .to_lowercase()
                    .contains(search.to_lowercase().as_str()),
                None => true,
            })
            .collect::<Vec<&String>>()
    }
}

impl Render for Menu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let elements = self.filter_elements();
        div()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .p_2()
            .when(elements.clone().is_empty(), |this| this.child("No element"))
            .children(elements.iter().enumerate().map(|(index, element)| {
                let element = (**element).clone();
                div()
                    .child(element.clone())
                    .hover(|this| this.bg(cx.theme().background.opacity(0.8)))
                    .id(SharedString::from(format!("element_{}", index)))
                    .on_click({
                        let element_for_closure = element.clone();
                        move |_, _, _| println!("Clicked {}", element_for_closure)
                    })
            }))
    }
}
