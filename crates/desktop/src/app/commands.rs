use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::Notification;
use gpui_kit::{App, Entity, Window};
use remindr_core::application::{Changes, Command, Created, SessionError};
use remindr_core::domain::{BlockId, DatabaseId, Parent};

use super::model::WorkspaceModel;
use super::navigation::{last_child, last_root_page};
use super::navigator::{Navigator, Route};

pub fn run(
    model: &Entity<WorkspaceModel>,
    command: Command,
    window: &mut Window,
    cx: &mut App,
) -> Option<Changes> {
    let outcome = model.update(cx, |model, cx| model.execute(command, cx));
    settle(outcome, window, cx)
}

pub fn undo(model: &Entity<WorkspaceModel>, window: &mut Window, cx: &mut App) {
    let outcome = model.update(cx, |model, cx| model.undo(cx));
    settle(outcome, window, cx);
}

pub fn redo(model: &Entity<WorkspaceModel>, window: &mut Window, cx: &mut App) {
    let outcome = model.update(cx, |model, cx| model.redo(cx));
    settle(outcome, window, cx);
}

pub fn create_root_page(
    model: &Entity<WorkspaceModel>,
    navigator: &Entity<Navigator>,
    window: &mut Window,
    cx: &mut App,
) {
    let after = last_root_page(model.read(cx).workspace());
    create_page(model, navigator, Parent::Root, after, window, cx);
}

pub fn create_child_page(
    model: &Entity<WorkspaceModel>,
    navigator: &Entity<Navigator>,
    parent: BlockId,
    window: &mut Window,
    cx: &mut App,
) {
    let after = last_child(model.read(cx).workspace(), parent);
    create_page(model, navigator, Parent::Block(parent), after, window, cx);
}

pub fn create_database(
    model: &Entity<WorkspaceModel>,
    navigator: &Entity<Navigator>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(changes) = run(model, Command::CreateDatabase, window, cx) else {
        return;
    };
    if let Some(database) = created_database(&changes) {
        navigator.update(cx, |navigator, cx| {
            navigator.open(Route::Database(database), cx);
        });
    }
}

fn create_page(
    model: &Entity<WorkspaceModel>,
    navigator: &Entity<Navigator>,
    parent: Parent,
    after: Option<BlockId>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(changes) = run(model, Command::CreatePage { parent, after }, window, cx) else {
        return;
    };
    if let Some(page) = created_block(&changes) {
        navigator.update(cx, |navigator, cx| navigator.open(Route::Page(page), cx));
    }
}

#[must_use]
pub fn created_block(changes: &Changes) -> Option<BlockId> {
    changes.created.iter().find_map(|created| match created {
        Created::Block(block) => Some(*block),
        _ => None,
    })
}

#[must_use]
pub fn created_database(changes: &Changes) -> Option<DatabaseId> {
    changes.created.iter().find_map(|created| match created {
        Created::Database(database) => Some(*database),
        _ => None,
    })
}

fn settle<T>(outcome: Result<T, SessionError>, window: &mut Window, cx: &mut App) -> Option<T> {
    match outcome {
        Ok(value) => Some(value),
        Err(error) => {
            window.push_notification(Notification::error(error.to_string()), cx);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::TestAppContext;

    use crate::app::testing::{Ui, failing_model};

    #[gpui_kit::test]
    fn failed_command_shows_a_notification_and_changes_nothing(cx: &mut TestAppContext) {
        let (model, store) = failing_model(cx);
        let ui = Ui::open(cx, &model);
        assert!(ui.find(cx, "notification").is_none());
        store.fail_next_commit();
        ui.click(cx, "new-page");
        assert!(ui.find(cx, "notification").is_some());
        assert_eq!(
            model.read_with(cx, |model, _| model
                .workspace()
                .children(remindr_core::domain::Parent::Root)
                .count()),
            0
        );
        assert_eq!(ui.route(cx), None);
        ui.click(cx, "new-page");
        assert_eq!(
            model.read_with(cx, |model, _| model
                .workspace()
                .children(remindr_core::domain::Parent::Root)
                .count()),
            1
        );
    }
}
