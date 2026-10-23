use gpui_kit::ElementId;
use remindr_core::domain::{BlockId, OptionId, PropertyId, Uuid, ViewId};

fn named(prefix: &str, id: Uuid) -> ElementId {
    ElementId::Name(format!("{prefix}-{id}").into())
}

fn paired(prefix: &str, first: Uuid, second: Uuid) -> ElementId {
    ElementId::Name(format!("{prefix}-{first}-{second}").into())
}

#[must_use]
pub fn block_id(block: BlockId) -> ElementId {
    named("database-view-block", block.as_uuid())
}

#[must_use]
pub fn cell_id(row: BlockId, property: PropertyId) -> ElementId {
    paired("cell", row.as_uuid(), property.as_uuid())
}

#[must_use]
pub fn open_row_id(row: BlockId) -> ElementId {
    named("open-row", row.as_uuid())
}

#[must_use]
pub fn row_id(row: BlockId) -> ElementId {
    named("table-row", row.as_uuid())
}

#[must_use]
pub fn header_id(property: PropertyId) -> ElementId {
    named("column-header", property.as_uuid())
}

#[must_use]
pub fn rename_id(property: PropertyId) -> ElementId {
    named("column-rename", property.as_uuid())
}

#[must_use]
pub fn option_id(option: OptionId) -> ElementId {
    named("option", option.as_uuid())
}

#[must_use]
pub fn card_id(row: BlockId) -> ElementId {
    named("card", row.as_uuid())
}

#[must_use]
pub fn group_id(option: Option<OptionId>) -> ElementId {
    match option {
        Some(option) => named("board-group", option.as_uuid()),
        None => ElementId::Name("board-group-none".into()),
    }
}

#[must_use]
pub fn new_card_id(option: Option<OptionId>) -> ElementId {
    match option {
        Some(option) => named("board-new", option.as_uuid()),
        None => ElementId::Name("board-new-none".into()),
    }
}

#[must_use]
pub fn tab_rename_id(view: ViewId) -> ElementId {
    named("view-rename", view.as_uuid())
}

#[must_use]
pub fn tab_label_id(view: ViewId) -> ElementId {
    named("view-tab", view.as_uuid())
}

#[must_use]
pub fn filter_row_id(index: usize) -> ElementId {
    indexed("filter-row", index)
}

#[must_use]
pub fn sort_row_id(index: usize) -> ElementId {
    indexed("sort-row", index)
}

fn indexed(prefix: &'static str, index: usize) -> ElementId {
    ElementId::NamedInteger(prefix.into(), index as u64)
}

#[must_use]
pub fn filter_property_id(index: usize) -> ElementId {
    indexed("filter-property", index)
}

#[must_use]
pub fn filter_condition_id(index: usize) -> ElementId {
    indexed("filter-condition", index)
}

#[must_use]
pub fn filter_value_id(index: usize) -> ElementId {
    indexed("filter-value", index)
}

#[must_use]
pub fn filter_remove_id(index: usize) -> ElementId {
    indexed("filter-remove", index)
}

#[must_use]
pub fn sort_property_id(index: usize) -> ElementId {
    indexed("sort-property", index)
}

#[must_use]
pub fn sort_direction_id(index: usize) -> ElementId {
    indexed("sort-direction", index)
}

#[must_use]
pub fn sort_remove_id(index: usize) -> ElementId {
    indexed("sort-remove", index)
}

#[must_use]
pub fn card_prop_id(row: BlockId, property: PropertyId) -> ElementId {
    paired("card-prop", row.as_uuid(), property.as_uuid())
}

#[must_use]
pub fn property_line_id(property: PropertyId) -> ElementId {
    named("row-property", property.as_uuid())
}

#[must_use]
pub fn grip_id(row: BlockId) -> ElementId {
    named("row-grip", row.as_uuid())
}
