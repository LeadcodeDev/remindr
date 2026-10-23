use std::cell::Cell;

use gpui_kit::component::Sizable as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{App, AppContext as _, Entity, ParentElement as _, Styled as _, Window, px};
use remindr_core::application::Command;
use remindr_core::domain::{BlockId, DatabaseId, RichText};

use crate::app::WorkspaceModel;
use crate::app::commands;
use crate::theme::{UI_TEXT, labeled};

#[derive(Clone, Copy)]
pub enum RenameTarget {
    Page(BlockId),
    Database(DatabaseId),
}

impl RenameTarget {
    fn title(self) -> &'static str {
        match self {
            Self::Page(_) => "Rename page",
            Self::Database(_) => "Rename database",
        }
    }

    fn command(self, name: &str) -> Command {
        match self {
            Self::Page(block) => Command::EditText {
                block,
                text: RichText::plain(name),
            },
            Self::Database(database) => Command::RenameDatabase {
                database,
                name: name.to_owned(),
            },
        }
    }
}

pub fn open(
    target: RenameTarget,
    model: &Entity<WorkspaceModel>,
    window: &mut Window,
    cx: &mut App,
) {
    let current = current_name(target, model, cx);
    let input = cx.new(|cx| InputState::new(window, cx).default_value(current));
    let first_build = Cell::new(true);
    let model = model.clone();
    window.open_dialog(cx, move |dialog, window, cx| {
        if first_build.replace(false) {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        let confirm_input = input.clone();
        let confirm_model = model.clone();
        dialog
            .title(target.title())
            .w(px(420.))
            .child(
                Input::new(&input)
                    .id("rename-input")
                    .small()
                    .text_size(UI_TEXT),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new()
                            .child(labeled(Button::new("rename-cancel").small(), "Cancel")),
                    )
                    .child(DialogAction::new().child(labeled(
                        Button::new("rename-ok").primary().small(),
                        "Rename",
                    ))),
            )
            .on_ok(move |_, window, cx| {
                let name = confirm_input.read(cx).value();
                let command = target.command(name.trim());
                commands::run(&confirm_model, command, window, cx).is_some()
            })
    });
}

fn current_name(target: RenameTarget, model: &Entity<WorkspaceModel>, cx: &App) -> String {
    let workspace = model.read(cx).workspace();
    match target {
        RenameTarget::Page(page) => workspace
            .block(page)
            .map(|block| block.text.to_plain())
            .unwrap_or_default(),
        RenameTarget::Database(database) => workspace
            .database(database)
            .map(|database| database.name.clone())
            .unwrap_or_default(),
    }
}
