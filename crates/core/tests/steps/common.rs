use cucumber::{given, then, when};
use remindr_core::application::{Changes, Command, Created};
use remindr_core::domain::Parent;

use crate::{RemindrWorld, run_feature};

#[given("an empty workspace")]
fn empty_workspace(_: &mut RemindrWorld) {}

#[given(expr = "a page {string}")]
fn a_page(world: &mut RemindrWorld, name: String) {
    let after = world
        .session
        .workspace()
        .children(Parent::Root)
        .last()
        .map(|block| block.id);
    world.run(Command::CreatePage {
        parent: Parent::Root,
        after,
    });
    let created = world.changes().created.clone();
    let Some(Created::Block(id)) = created.first().copied() else {
        panic!("page {name} was not created");
    };
    world.blocks.insert(name, id);
}

#[given("the next commit fails")]
fn next_commit_fails(world: &mut RemindrWorld) {
    world.store.fail_next_commit();
}

#[given(expr = "{int} ms pass")]
#[when(expr = "{int} ms pass")]
fn time_passes(world: &mut RemindrWorld, ms: i64) {
    world.clock.advance(ms);
}

#[when("I undo")]
fn undo(world: &mut RemindrWorld) {
    world.before = Some(world.session.workspace().to_snapshot());
    world.last = Some(
        world
            .session
            .undo()
            .map(Option::unwrap_or_default)
            .map_err(|error| error.to_string()),
    );
}

#[when("I redo")]
fn redo(world: &mut RemindrWorld) {
    world.before = Some(world.session.workspace().to_snapshot());
    world.last = Some(
        world
            .session
            .redo()
            .map(Option::unwrap_or_default)
            .map_err(|error| error.to_string()),
    );
}

#[then("the command succeeds")]
fn command_succeeds(world: &mut RemindrWorld) {
    let _: &Changes = world.changes();
}

#[then("the command is rejected")]
fn command_rejected(world: &mut RemindrWorld) {
    assert!(
        matches!(world.last, Some(Err(_))),
        "expected a rejection, got {:?}",
        world.last
    );
}

#[then("the workspace is unchanged")]
fn workspace_unchanged(world: &mut RemindrWorld) {
    let before = world.before.as_ref().expect("a command ran before");
    assert_eq!(&world.session.workspace().to_snapshot(), before);
}

#[then("the workspace has no pages")]
fn no_pages(world: &mut RemindrWorld) {
    assert_eq!(world.session.workspace().children(Parent::Root).count(), 0);
}

#[test]
fn workspace_scenarios() {
    run_feature("tests/features/workspace.feature");
}
