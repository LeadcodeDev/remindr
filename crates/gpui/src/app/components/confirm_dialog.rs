use std::rc::Rc;

use gpui::{
    App, ClickEvent, Element, IntoElement, ParentElement, Styled, Window, prelude::FluentBuilder,
    px,
};
use gpui_component::{
    ActiveTheme, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    v_flex,
};

/// A confirmation dialog for dangerous actions (e.g., file deletion).
///
/// # Example
///
/// ```rust
/// ConfirmDialog::new("Delete Document")
///     .message("Are you sure you want to delete this document? This action cannot be undone.")
///     .danger()
///     .on_confirm(|window, cx| {
///         // Perform the dangerous action
///         true // Return true to close the dialog
///     })
///     .open(window, cx);
/// ```
pub struct ConfirmDialog {
    title: String,
    message: Option<String>,
    confirm_text: String,
    cancel_text: String,
    is_danger: bool,
    on_confirm: Option<Rc<dyn Fn(&mut Window, &mut App) -> bool + 'static>>,
}

impl ConfirmDialog {
    /// Create a new confirmation dialog with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: None,
            confirm_text: "Confirm".into(),
            cancel_text: "Cancel".into(),
            is_danger: false,
            on_confirm: None,
        }
    }

    /// Set the message to display in the dialog.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set the confirm button text. Default is "Confirm".
    pub fn confirm_text(mut self, text: impl Into<String>) -> Self {
        self.confirm_text = text.into();
        self
    }

    /// Set the cancel button text. Default is "Cancel".
    pub fn cancel_text(mut self, text: impl Into<String>) -> Self {
        self.cancel_text = text.into();
        self
    }

    /// Mark this dialog as a dangerous action (uses destructive button style).
    pub fn danger(mut self) -> Self {
        self.is_danger = true;
        self
    }

    /// Set the callback for when the user confirms.
    /// Return `true` to close the dialog, `false` to keep it open.
    pub fn on_confirm<F>(mut self, callback: F) -> Self
    where
        F: Fn(&mut Window, &mut App) -> bool + 'static,
    {
        self.on_confirm = Some(Rc::new(callback));
        self
    }

    /// Open the confirmation dialog.
    pub fn open(self, window: &mut Window, cx: &mut App) {
        let on_confirm = self.on_confirm;
        let message = self.message;
        let title = self.title;
        let confirm_text = self.confirm_text;
        let cancel_text = self.cancel_text;
        let is_danger = self.is_danger;

        window.open_dialog(cx, move |dialog, _window, cx| {
            let on_confirm_clone = on_confirm.clone();
            let cancel_text_clone = cancel_text.clone();
            let confirm_text_clone = confirm_text.clone();

            dialog
                .w(px(280.))
                .pt(px(12.))
                .pb(px(12.))
                .px(px(14.))
                .title(v_flex().text_sm().font_semibold().child(title.clone()))
                .close_button(false)
                .overlay_closable(true)
                .footer(move |_ok_btn, _cancel_btn, _window, _cx| {
                    let on_confirm = on_confirm_clone.clone();
                    let cancel_text = cancel_text_clone.clone();
                    let confirm_text = confirm_text_clone.clone();

                    vec![
                        Button::new("cancel")
                            .small()
                            .ghost()
                            .label(cancel_text)
                            .on_click({
                                move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                                    window.close_dialog(cx);
                                }
                            })
                            .into_element()
                            .into_any(),
                        Button::new("confirm")
                            .small()
                            .when(is_danger, |btn| btn.danger())
                            .when(!is_danger, |btn| btn.primary())
                            .label(confirm_text)
                            .on_click({
                                let on_confirm = on_confirm.clone();
                                move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                                    let should_close = if let Some(ref callback) = on_confirm {
                                        callback(window, cx)
                                    } else {
                                        true
                                    };
                                    if should_close {
                                        window.close_dialog(cx);
                                    }
                                }
                            })
                            .into_element()
                            .into_any(),
                    ]
                })
                .child(
                    v_flex()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .when_some(message.clone(), |this, msg| this.child(msg)),
                )
        });
    }
}
