use gpui_kit::component::{Icon, IconName, h_flex};
use gpui_kit::{Div, ParentElement as _, SharedString, Styled as _};

pub fn icon_label(icon: IconName, text: impl Into<SharedString>) -> Div {
    h_flex()
        .gap_1p5()
        .items_center()
        .child(Icon::new(icon).size_3p5())
        .child(text.into())
}
