use gpui_kit::component::menu::PopupMenu;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{ActiveTheme as _, Selectable, h_flex};
use gpui_kit::{
    AnyElement, App, Context, DismissEvent, ElementId, Entity, Focusable as _,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _,
    Window, prelude::FluentBuilder as _, px,
};

const HEADER_HEIGHT: f32 = 32.;

#[derive(Default)]
pub struct MenuState {
    open: bool,
    pressed_while_open: bool,
    menu: Option<(Entity<PopupMenu>, Subscription)>,
}

impl MenuState {
    pub fn is_open(&self) -> bool {
        self.open
    }

    fn press(&mut self, was_open: bool) {
        self.pressed_while_open = was_open;
    }

    fn click(&mut self, cx: &mut Context<Self>) {
        self.open = !self.pressed_while_open;
        if !self.open {
            self.menu = None;
        }
        cx.notify();
    }

    fn menu<B>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        builder: &B,
    ) -> Entity<PopupMenu>
    where
        B: Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu,
    {
        if let Some((menu, _)) = &self.menu {
            return menu.clone();
        }
        let menu = PopupMenu::build(window, cx, |menu, window, cx| builder(menu, window, cx));
        menu.focus_handle(cx).focus(window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.open = false;
            this.menu = None;
            cx.notify();
        });
        self.menu = Some((menu.clone(), subscription));
        menu
    }
}

#[derive(IntoElement)]
struct Trigger {
    id: ElementId,
    label: SharedString,
    open: bool,
    state: Entity<MenuState>,
    children: Vec<AnyElement>,
}

impl gpui_kit::ParentElement for Trigger {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Selectable for Trigger {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl RenderOnce for Trigger {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let was_open = self.open;
        let pressed = self.state.clone();
        let clicked = self.state;
        h_flex()
            .id(self.id)
            .test_support()
            .aria_label(self.label)
            .aria_expanded(self.open)
            .w_full()
            .h(px(HEADER_HEIGHT))
            .px_2()
            .gap_1p5()
            .items_center()
            .overflow_hidden()
            .cursor_pointer()
            .when(self.open, |this| this.bg(cx.theme().accent))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                pressed.update(cx, |state, _| state.press(was_open));
            })
            .on_click(move |_, _, cx| {
                clicked.update(cx, |state, cx| state.click(cx));
            })
            .children(self.children)
    }
}

pub fn cell<B>(
    id: ElementId,
    label: SharedString,
    state: &Entity<MenuState>,
    open: bool,
    children: Vec<AnyElement>,
    builder: B,
) -> AnyElement
where
    B: Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
{
    let trigger = Trigger {
        id: id.clone(),
        label,
        open,
        state: state.clone(),
        children,
    };
    let owner = state.clone();
    Popover::new(ElementId::Name(format!("menu-{id}").into()))
        .open(open)
        .appearance(false)
        .overlay_closable(false)
        .mouse_button(MouseButton::Middle)
        .trigger(trigger)
        .content(move |_, window, cx| {
            owner.update(cx, |state, cx| state.menu(window, cx, &builder))
        })
        .w_full()
        .into_any_element()
}
