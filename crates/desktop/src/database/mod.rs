mod block;
mod board;
mod cell;
mod changes;
mod conditions;
mod filter;
mod format;
mod header;
mod ids;
mod kinds;
mod row;
mod screen;
mod sort;
mod table;
mod tags;
#[cfg(test)]
mod testing;
mod toolbar;
mod ui;

use gpui_kit::{AnyView, App, AppContext as _, Entity, Window};
use remindr_core::domain::{BlockId, DatabaseId};

use crate::app::WorkspaceModel;
use crate::embed::EmbedRegistry;

pub use block::DatabaseViewBlock;
pub use row::RowProperties;
pub use screen::DatabaseScreen;

pub fn init(cx: &mut App) {
    cx.default_global::<EmbedRegistry>().register(embed);
}

fn embed(
    model: Entity<WorkspaceModel>,
    block: BlockId,
    database: DatabaseId,
    window: &mut Window,
    cx: &mut App,
) -> AnyView {
    cx.new(|cx| DatabaseViewBlock::new(model, block, database, window, cx))
        .into()
}
