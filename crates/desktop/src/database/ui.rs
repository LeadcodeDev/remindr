use gpui_kit::component::{Icon, IconName, h_flex};
use gpui_kit::{App, Div, ParentElement as _, SharedString, Styled as _};
use remindr_core::domain::BlockId;

use crate::app::{NavigatorHandle, Route};
use crate::theme::UI_TEXT;

pub fn icon_label(icon: IconName, text: impl Into<SharedString>) -> Div {
    h_flex()
        .gap_1p5()
        .items_center()
        .text_size(UI_TEXT)
        .child(Icon::new(icon).size_3p5())
        .child(text.into())
}

pub fn open_page(row: BlockId, cx: &mut App) {
    let Some(navigator) = cx
        .try_global::<NavigatorHandle>()
        .map(|handle| handle.0.clone())
    else {
        return;
    };
    navigator.update(cx, |navigator, cx| navigator.open(Route::Page(row), cx));
}
