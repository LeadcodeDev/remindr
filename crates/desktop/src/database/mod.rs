use gpui_kit::{
    App, Context, Entity, IntoElement, Render, TestSupportExt, Window, div, prelude::*,
};
use remindr_core::domain::{BlockId, DatabaseId};

use crate::app::WorkspaceModel;

pub fn init(_: &mut App) {}

pub struct DatabaseViewBlock {
    model: Entity<WorkspaceModel>,
    block: BlockId,
    database: DatabaseId,
}

impl DatabaseViewBlock {
    pub fn new(
        model: Entity<WorkspaceModel>,
        block: BlockId,
        database: DatabaseId,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Self {
        Self {
            model,
            block,
            database,
        }
    }
}

impl Render for DatabaseViewBlock {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("database-view-block").test_support()
    }
}

pub struct DatabaseScreen {
    model: Entity<WorkspaceModel>,
    database: DatabaseId,
}

impl DatabaseScreen {
    pub fn new(
        model: Entity<WorkspaceModel>,
        database: DatabaseId,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Self {
        Self { model, database }
    }
}

impl Render for DatabaseScreen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("database-screen").test_support()
    }
}

pub struct RowProperties {
    model: Entity<WorkspaceModel>,
    row: BlockId,
}

impl RowProperties {
    pub fn new(
        model: Entity<WorkspaceModel>,
        row: BlockId,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Self {
        Self { model, row }
    }
}

impl Render for RowProperties {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("row-properties").test_support()
    }
}
