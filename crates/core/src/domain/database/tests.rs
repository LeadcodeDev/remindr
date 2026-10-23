use proptest::prelude::*;

use super::fixture::{Sample, block, database, key, option, property, property_id, view};
use super::{CalendarDate, PropertyKind, PropertyValue};
use crate::domain::{
    BlockId, BlockKind, Condition, DatabaseId, DomainError, Emoji, Filter, MultiSelectCondition,
    NumberCondition, NumberFormat, Operation, OptionId, Parent, Property, PropertyId,
    SelectCondition, SelectOption, Sort, SortDirection, TextCondition, Transaction, Uuid, View,
    ViewId, ViewKind, Workspace, WorkspaceSnapshot,
};

fn assert_rejected(workspace: &mut Workspace, operation: Operation, expected: DomainError) {
    let before = workspace.clone();
    assert_eq!(
        workspace.apply_database_operation(&operation),
        Err(expected)
    );
    assert_eq!(*workspace, before);
}

fn set_value(row: BlockId, property: PropertyId, value: Option<PropertyValue>) -> Operation {
    Operation::SetValue {
        row,
        property,
        value,
    }
}

fn put_property(database: DatabaseId, property: Property, index: usize) -> Operation {
    Operation::PutProperty {
        database,
        property,
        index,
    }
}

fn edited_view(sample: &Sample, change: impl FnOnce(&mut View)) -> View {
    let mut edited = sample.workspace.view(sample.view).unwrap().clone();
    change(&mut edited);
    edited
}

fn property_of(sample: &Sample, id: PropertyId) -> Property {
    sample
        .workspace
        .database(sample.database)
        .unwrap()
        .schema
        .properties
        .iter()
        .find(|property| property.id == id)
        .unwrap()
        .clone()
}

fn property_ids(workspace: &Workspace, database: DatabaseId) -> Vec<PropertyId> {
    workspace
        .database(database)
        .unwrap()
        .schema
        .properties
        .iter()
        .map(|property| property.id)
        .collect()
}

fn other_database(sample: &mut Sample) -> (DatabaseId, PropertyId) {
    let id = DatabaseId::from_uuid(Uuid::from_u128(101));
    let title = property_id(50);
    let created = database(
        id,
        "Other",
        vec![property(title, "Name", PropertyKind::Title)],
    );
    sample
        .workspace
        .apply(&Transaction::new(vec![Operation::InsertDatabase(created)]))
        .unwrap();
    (id, title)
}

#[test]
fn calendar_date_follows_the_gregorian_calendar() {
    assert!(CalendarDate::new(2024, 2, 29).is_ok());
    assert!(CalendarDate::new(2023, 2, 29).is_err());
    assert!(CalendarDate::new(1900, 2, 29).is_err());
    assert!(CalendarDate::new(2000, 2, 29).is_ok());
    assert!(CalendarDate::new(2026, 4, 31).is_err());
    assert!(CalendarDate::new(2026, 13, 1).is_err());
    assert!(CalendarDate::new(2026, 1, 0).is_err());
}

#[test]
fn insert_database_adds_it_and_the_inverse_removes_it() {
    let mut sample = Sample::new();
    let before = sample.workspace.clone();
    let id = DatabaseId::from_uuid(Uuid::from_u128(101));
    let created = database(
        id,
        "Other",
        vec![property(property_id(50), "Name", PropertyKind::Title)],
    );
    let inverse = sample
        .workspace
        .apply_database_operation(&Operation::InsertDatabase(created.clone()))
        .unwrap();
    assert_eq!(sample.workspace.database(id), Some(&created));
    assert_eq!(inverse, Operation::RemoveDatabase { database: id });
    sample.workspace.apply_database_operation(&inverse).unwrap();
    assert_eq!(sample.workspace, before);
}

#[test]
fn insert_database_with_existing_id_is_already_exists() {
    let mut sample = Sample::new();
    let existing = sample.workspace.database(sample.database).unwrap().clone();
    assert_rejected(
        &mut sample.workspace,
        Operation::InsertDatabase(existing),
        DomainError::AlreadyExists,
    );
}

#[test]
fn insert_database_rejects_a_schema_without_exactly_one_title() {
    let mut sample = Sample::new();
    let id = DatabaseId::from_uuid(Uuid::from_u128(101));
    let none = database(id, "None", Vec::new());
    assert_rejected(
        &mut sample.workspace,
        Operation::InsertDatabase(none),
        DomainError::TitleIsBlockText,
    );
    let two = database(
        id,
        "Two",
        vec![
            property(property_id(50), "A", PropertyKind::Title),
            property(property_id(51), "B", PropertyKind::Title),
        ],
    );
    assert_rejected(
        &mut sample.workspace,
        Operation::InsertDatabase(two),
        DomainError::TitleIsBlockText,
    );
}

#[test]
fn insert_database_rejects_a_property_id_used_elsewhere() {
    let mut sample = Sample::new();
    let id = DatabaseId::from_uuid(Uuid::from_u128(101));
    let clash = database(
        id,
        "Clash",
        vec![
            property(property_id(50), "A", PropertyKind::Title),
            property(sample.notes, "B", PropertyKind::Text),
        ],
    );
    assert_rejected(
        &mut sample.workspace,
        Operation::InsertDatabase(clash),
        DomainError::AlreadyExists,
    );
    let twice = database(
        id,
        "Twice",
        vec![
            property(property_id(50), "A", PropertyKind::Title),
            property(property_id(51), "B", PropertyKind::Text),
            property(property_id(51), "C", PropertyKind::Text),
        ],
    );
    assert_rejected(
        &mut sample.workspace,
        Operation::InsertDatabase(twice),
        DomainError::AlreadyExists,
    );
}

#[test]
fn remove_database_rejects_an_unknown_database() {
    let mut sample = Sample::new();
    let unknown = DatabaseId::new();
    assert_rejected(
        &mut sample.workspace,
        Operation::RemoveDatabase { database: unknown },
        DomainError::UnknownDatabase(unknown),
    );
}

#[test]
fn remove_database_with_rows_is_still_referenced() {
    let mut sample = Sample::new();
    let (other, _) = other_database(&mut sample);
    let row = block(
        BlockId::new(),
        BlockKind::Page { icon: None },
        Parent::Database(other),
        key(1),
    );
    sample
        .workspace
        .children
        .entry(row.parent)
        .or_default()
        .insert((row.sort_key.clone(), row.id));
    sample.workspace.blocks.insert(row.id, row);
    assert_rejected(
        &mut sample.workspace,
        Operation::RemoveDatabase { database: other },
        DomainError::StillReferenced,
    );
}

#[test]
fn remove_database_rejects_a_database_shown_by_a_block() {
    let mut sample = Sample::new();
    let database = sample.database;
    assert_rejected(
        &mut sample.workspace,
        Operation::RemoveDatabase { database },
        DomainError::StillReferenced,
    );
}

#[test]
fn remove_database_rejects_a_database_that_still_has_values() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    sample.set(row, sample.notes, PropertyValue::Text("x".to_owned()));
    sample.workspace.blocks.remove(&row);
    sample.workspace.views.clear();
    sample.workspace.blocks.remove(&sample.embed);
    let database = sample.database;
    assert_rejected(
        &mut sample.workspace,
        Operation::RemoveDatabase { database },
        DomainError::StillReferenced,
    );
}

#[test]
fn remove_database_removes_an_unused_database_and_the_inverse_restores_it() {
    let mut sample = Sample::new();
    let (other, _) = other_database(&mut sample);
    let before = sample.workspace.clone();
    let inverse = sample
        .workspace
        .apply_database_operation(&Operation::RemoveDatabase { database: other })
        .unwrap();
    assert!(sample.workspace.database(other).is_none());
    sample.workspace.apply_database_operation(&inverse).unwrap();
    assert_eq!(sample.workspace, before);
}

#[test]
fn database_attribute_operations_reject_an_unknown_database() {
    let mut sample = Sample::new();
    let unknown = DatabaseId::new();
    let expected = DomainError::UnknownDatabase(unknown);
    assert_rejected(
        &mut sample.workspace,
        Operation::SetDatabaseName {
            database: unknown,
            name: "x".to_owned(),
        },
        expected,
    );
    assert_rejected(
        &mut sample.workspace,
        Operation::SetDatabaseIcon {
            database: unknown,
            icon: None,
        },
        DomainError::UnknownDatabase(unknown),
    );
    assert_rejected(
        &mut sample.workspace,
        Operation::SetDatabaseAlive {
            database: unknown,
            alive: false,
        },
        DomainError::UnknownDatabase(unknown),
    );
}

#[test]
fn database_attribute_operations_return_the_old_value() {
    let mut sample = Sample::new();
    let database = sample.database;
    let icon = Emoji::try_from("📋").unwrap();
    let operations = [
        Operation::SetDatabaseName {
            database,
            name: "Renamed".to_owned(),
        },
        Operation::SetDatabaseIcon {
            database,
            icon: Some(icon),
        },
        Operation::SetDatabaseAlive {
            database,
            alive: false,
        },
    ];
    for operation in operations {
        let before = sample.workspace.clone();
        let inverse = sample
            .workspace
            .apply_database_operation(&operation)
            .unwrap();
        assert_ne!(sample.workspace, before);
        sample.workspace.apply_database_operation(&inverse).unwrap();
        assert_eq!(sample.workspace, before);
    }
    let stored = sample.workspace.database(sample.database).unwrap();
    assert_eq!(stored.name, "Tasks");
    assert!(stored.alive);
}

#[test]
fn put_property_rejects_an_unknown_database() {
    let mut sample = Sample::new();
    let unknown = DatabaseId::new();
    let extra = property(property_id(60), "Extra", PropertyKind::Text);
    assert_rejected(
        &mut sample.workspace,
        put_property(unknown, extra, 0),
        DomainError::UnknownDatabase(unknown),
    );
}

#[test]
fn put_property_keeps_exactly_one_title() {
    let mut sample = Sample::new();
    let database = sample.database;
    let second = property(property_id(60), "Second", PropertyKind::Title);
    assert_rejected(
        &mut sample.workspace,
        put_property(database, second, 0),
        DomainError::TitleIsBlockText,
    );
    let promoted = property(sample.notes, "Notes", PropertyKind::Title);
    assert_rejected(
        &mut sample.workspace,
        put_property(database, promoted, 0),
        DomainError::TitleIsBlockText,
    );
    let demoted = property(sample.title, "Name", PropertyKind::Text);
    assert_rejected(
        &mut sample.workspace,
        put_property(database, demoted, 0),
        DomainError::TitleIsBlockText,
    );
}

#[test]
fn put_property_rejects_a_property_id_used_by_another_database() {
    let mut sample = Sample::new();
    let (other, _) = other_database(&mut sample);
    let clash = property(sample.notes, "Notes", PropertyKind::Text);
    assert_rejected(
        &mut sample.workspace,
        put_property(other, clash, 1),
        DomainError::AlreadyExists,
    );
}

#[test]
fn put_property_rejects_a_kind_change_while_values_exist() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    sample.set(row, sample.notes, PropertyValue::Text("x".to_owned()));
    let database = sample.database;
    let retyped = property(sample.notes, "Notes", PropertyKind::Checkbox);
    assert_rejected(
        &mut sample.workspace,
        put_property(database, retyped, 7),
        DomainError::ValueKindMismatch,
    );
}

#[test]
fn put_property_allows_a_kind_change_without_values() {
    let mut sample = Sample::new();
    let database = sample.database;
    let retyped = property(sample.notes, "Notes", PropertyKind::Checkbox);
    sample
        .workspace
        .apply_database_operation(&put_property(database, retyped.clone(), 7))
        .unwrap();
    assert_eq!(property_of(&sample, sample.notes), retyped);
}

#[test]
fn put_property_rejects_removing_an_option_that_values_reference() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    sample.set(row, sample.status, PropertyValue::Select(sample.doing));
    let database = sample.database;
    let fewer = property(
        sample.status,
        "Status",
        PropertyKind::Select {
            options: vec![option(sample.todo, "Todo")],
        },
    );
    assert_rejected(
        &mut sample.workspace,
        put_property(database, fewer, 1),
        DomainError::UnknownOption(sample.doing),
    );
}

#[test]
fn put_property_allows_removing_an_option_nobody_uses() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    sample.set(row, sample.status, PropertyValue::Select(sample.todo));
    let database = sample.database;
    let fewer = property(
        sample.status,
        "Status",
        PropertyKind::Select {
            options: vec![option(sample.todo, "Todo")],
        },
    );
    sample
        .workspace
        .apply_database_operation(&put_property(database, fewer.clone(), 1))
        .unwrap();
    assert_eq!(property_of(&sample, sample.status), fewer);
}

#[test]
fn put_property_rejects_a_kind_that_breaks_a_view() {
    let mut sample = Sample::new();
    let filter = Filter {
        property: sample.notes,
        condition: Condition::Text(TextCondition::IsEmpty),
    };
    sample.view_mut().filters.push(filter);
    let database = sample.database;
    let retyped = property(
        sample.notes,
        "Notes",
        PropertyKind::Number {
            format: NumberFormat::Plain,
        },
    );
    assert_rejected(
        &mut sample.workspace,
        put_property(database, retyped, 7),
        DomainError::ConditionKindMismatch,
    );
    sample.view_mut().filters.clear();
    sample.view_mut().kind = ViewKind::Board {
        group_by: sample.status,
    };
    let retyped = property(
        sample.status,
        "Status",
        PropertyKind::MultiSelect {
            options: Vec::new(),
        },
    );
    assert_rejected(
        &mut sample.workspace,
        put_property(database, retyped, 1),
        DomainError::ValueKindMismatch,
    );
}

#[test]
fn put_property_inserts_at_the_index_clamped_to_the_length() {
    let mut sample = Sample::new();
    let database = sample.database;
    let first = property(property_id(60), "First", PropertyKind::Text);
    let last = property(property_id(61), "Last", PropertyKind::Text);
    sample
        .workspace
        .apply_database_operation(&put_property(database, first, 0))
        .unwrap();
    sample
        .workspace
        .apply_database_operation(&put_property(database, last, 99))
        .unwrap();
    let ids = property_ids(&sample.workspace, database);
    assert_eq!(ids.first(), Some(&property_id(60)));
    assert_eq!(ids.last(), Some(&property_id(61)));
    assert_eq!(ids.len(), 10);
}

#[test]
fn put_property_moves_an_existing_property() {
    let mut sample = Sample::new();
    let database = sample.database;
    let notes = property_of(&sample, sample.notes);
    sample
        .workspace
        .apply_database_operation(&put_property(database, notes, 1))
        .unwrap();
    let ids = property_ids(&sample.workspace, database);
    assert_eq!(ids[1], sample.notes);
    assert_eq!(ids.len(), 8);
}

#[test]
fn put_property_returns_remove_for_a_new_property_and_put_for_an_existing_one() {
    let mut sample = Sample::new();
    let database = sample.database;
    let before = sample.workspace.clone();
    let fresh = property(property_id(60), "Fresh", PropertyKind::Text);
    let inverse = sample
        .workspace
        .apply_database_operation(&put_property(database, fresh, 2))
        .unwrap();
    assert_eq!(
        inverse,
        Operation::RemoveProperty {
            database,
            property: property_id(60)
        }
    );
    sample.workspace.apply_database_operation(&inverse).unwrap();
    assert_eq!(sample.workspace, before);
    let old = property_of(&sample, sample.link);
    let renamed = property(sample.link, "Site", PropertyKind::Url);
    let inverse = sample
        .workspace
        .apply_database_operation(&put_property(database, renamed, 0))
        .unwrap();
    assert_eq!(inverse, put_property(database, old, 6));
    sample.workspace.apply_database_operation(&inverse).unwrap();
    assert_eq!(sample.workspace, before);
}

#[test]
fn remove_property_rejects_unknown_ids() {
    let mut sample = Sample::new();
    let unknown_database = DatabaseId::new();
    assert_rejected(
        &mut sample.workspace,
        Operation::RemoveProperty {
            database: unknown_database,
            property: sample.notes,
        },
        DomainError::UnknownDatabase(unknown_database),
    );
    let unknown_property = property_id(99);
    let database = sample.database;
    assert_rejected(
        &mut sample.workspace,
        Operation::RemoveProperty {
            database,
            property: unknown_property,
        },
        DomainError::UnknownProperty(unknown_property),
    );
}

#[test]
fn remove_property_rejects_the_title() {
    let mut sample = Sample::new();
    let database = sample.database;
    assert_rejected(
        &mut sample.workspace,
        Operation::RemoveProperty {
            database,
            property: sample.title,
        },
        DomainError::TitleIsBlockText,
    );
}

#[test]
fn remove_property_rejects_a_property_that_still_has_values() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    sample.set(row, sample.notes, PropertyValue::Text("x".to_owned()));
    let database = sample.database;
    assert_rejected(
        &mut sample.workspace,
        Operation::RemoveProperty {
            database,
            property: sample.notes,
        },
        DomainError::StillReferenced,
    );
}

#[test]
fn remove_property_rejects_a_property_a_view_references() {
    let references: [fn(&mut View, PropertyId); 4] = [
        |view, property| {
            view.filters.push(Filter {
                property,
                condition: Condition::Text(TextCondition::IsEmpty),
            });
        },
        |view, property| {
            view.sorts.push(Sort {
                property,
                direction: SortDirection::Ascending,
            });
        },
        |view, property| view.visible = vec![property],
        |view, property| {
            view.column_widths.insert(property, 120);
        },
    ];
    for reference in references {
        let mut sample = Sample::new();
        let database = sample.database;
        let notes = sample.notes;
        reference(sample.view_mut(), notes);
        assert_rejected(
            &mut sample.workspace,
            Operation::RemoveProperty {
                database,
                property: notes,
            },
            DomainError::StillReferenced,
        );
    }
    let mut sample = Sample::new();
    let database = sample.database;
    sample.view_mut().kind = ViewKind::Board {
        group_by: sample.status,
    };
    sample.view_mut().visible.clear();
    assert_rejected(
        &mut sample.workspace,
        Operation::RemoveProperty {
            database,
            property: sample.status,
        },
        DomainError::StillReferenced,
    );
}

#[test]
fn remove_property_removes_it_and_the_inverse_restores_its_position() {
    let mut sample = Sample::new();
    let database = sample.database;
    sample.view_mut().visible.clear();
    let before = sample.workspace.clone();
    let inverse = sample
        .workspace
        .apply_database_operation(&Operation::RemoveProperty {
            database,
            property: sample.amount,
        })
        .unwrap();
    assert!(!property_ids(&sample.workspace, database).contains(&sample.amount));
    sample.workspace.apply_database_operation(&inverse).unwrap();
    assert_eq!(sample.workspace, before);
}

#[test]
fn set_value_rejects_an_unknown_row_or_a_row_outside_a_database() {
    let mut sample = Sample::new();
    let unknown = BlockId::new();
    assert_rejected(
        &mut sample.workspace,
        set_value(unknown, sample.notes, None),
        DomainError::UnknownBlock(unknown),
    );
    assert_rejected(
        &mut sample.workspace,
        set_value(sample.page, sample.notes, None),
        DomainError::InvalidParent,
    );
    assert_rejected(
        &mut sample.workspace,
        set_value(sample.embed, sample.notes, None),
        DomainError::InvalidParent,
    );
}

#[test]
fn set_value_rejects_a_property_of_another_database() {
    let mut sample = Sample::new();
    let (_, foreign) = other_database(&mut sample);
    let row = sample.add_row("a");
    assert_rejected(
        &mut sample.workspace,
        set_value(row, foreign, None),
        DomainError::UnknownProperty(foreign),
    );
    let unknown = property_id(99);
    assert_rejected(
        &mut sample.workspace,
        set_value(row, unknown, None),
        DomainError::UnknownProperty(unknown),
    );
}

#[test]
fn set_value_on_title_is_rejected() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    assert_rejected(
        &mut sample.workspace,
        set_value(row, sample.title, Some(PropertyValue::Text("b".to_owned()))),
        DomainError::TitleIsBlockText,
    );
    assert_rejected(
        &mut sample.workspace,
        set_value(row, sample.title, None),
        DomainError::TitleIsBlockText,
    );
}

#[test]
fn set_value_rejects_a_value_of_the_wrong_kind() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    let date = PropertyValue::Date(CalendarDate::new(2026, 10, 23).unwrap());
    let cases = [
        (sample.notes, PropertyValue::Number(1.0)),
        (sample.link, PropertyValue::Text("x".to_owned())),
        (sample.amount, PropertyValue::Text("1".to_owned())),
        (sample.status, PropertyValue::MultiSelect(vec![sample.todo])),
        (sample.tags, PropertyValue::Select(sample.red)),
        (sample.due, PropertyValue::Checkbox(true)),
        (sample.done, date),
    ];
    for (property, value) in cases {
        assert_rejected(
            &mut sample.workspace,
            set_value(row, property, Some(value)),
            DomainError::ValueKindMismatch,
        );
    }
}

#[test]
fn nan_number_is_rejected() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_rejected(
            &mut sample.workspace,
            set_value(row, sample.amount, Some(PropertyValue::Number(number))),
            DomainError::ValueKindMismatch,
        );
    }
}

#[test]
fn set_value_rejects_an_unknown_option() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    let unknown = OptionId::new();
    assert_rejected(
        &mut sample.workspace,
        set_value(row, sample.status, Some(PropertyValue::Select(unknown))),
        DomainError::UnknownOption(unknown),
    );
    assert_rejected(
        &mut sample.workspace,
        set_value(
            row,
            sample.tags,
            Some(PropertyValue::MultiSelect(vec![sample.red, unknown])),
        ),
        DomainError::UnknownOption(unknown),
    );
}

#[test]
fn set_value_stores_each_kind() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    let date = PropertyValue::Date(CalendarDate::new(2026, 10, 23).unwrap());
    let cases = [
        (sample.notes, PropertyValue::Text("note".to_owned())),
        (sample.link, PropertyValue::Url("https://x.dev".to_owned())),
        (sample.amount, PropertyValue::Number(42.5)),
        (sample.status, PropertyValue::Select(sample.finished)),
        (
            sample.tags,
            PropertyValue::MultiSelect(vec![sample.red, sample.blue]),
        ),
        (sample.due, date),
        (sample.done, PropertyValue::Checkbox(true)),
    ];
    for (property, value) in cases {
        sample
            .workspace
            .apply_database_operation(&set_value(row, property, Some(value.clone())))
            .unwrap();
        assert_eq!(sample.workspace.value(row, property), Some(&value));
    }
}

#[test]
fn set_value_returns_the_old_value_and_none_clears() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    let first = PropertyValue::Number(1.0);
    let second = PropertyValue::Number(2.0);
    let inverse = sample
        .workspace
        .apply_database_operation(&set_value(row, sample.amount, Some(first.clone())))
        .unwrap();
    assert_eq!(inverse, set_value(row, sample.amount, None));
    let inverse = sample
        .workspace
        .apply_database_operation(&set_value(row, sample.amount, Some(second)))
        .unwrap();
    assert_eq!(inverse, set_value(row, sample.amount, Some(first.clone())));
    sample
        .workspace
        .apply_database_operation(&set_value(row, sample.amount, None))
        .unwrap();
    assert_eq!(sample.workspace.value(row, sample.amount), None);
    sample.workspace.apply_database_operation(&inverse).unwrap();
    assert_eq!(sample.workspace.value(row, sample.amount), Some(&first));
}

#[test]
fn set_value_accepts_a_dead_row() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    sample.workspace.blocks.get_mut(&row).unwrap().alive = false;
    sample
        .workspace
        .apply_database_operation(&set_value(
            row,
            sample.amount,
            Some(PropertyValue::Number(1.0)),
        ))
        .unwrap();
}

#[test]
fn put_view_adds_a_view_and_the_inverse_removes_it() {
    let mut sample = Sample::new();
    let before = sample.workspace.clone();
    let added = view(ViewId::new(), sample.embed, vec![sample.title]);
    let inverse = sample
        .workspace
        .apply_database_operation(&Operation::PutView(added.clone()))
        .unwrap();
    assert_eq!(sample.workspace.view(added.id), Some(&added));
    assert_eq!(inverse, Operation::RemoveView { view: added.id });
    sample.workspace.apply_database_operation(&inverse).unwrap();
    assert_eq!(sample.workspace, before);
}

#[test]
fn put_view_replaces_a_view_and_the_inverse_restores_it() {
    let mut sample = Sample::new();
    let before = sample.workspace.clone();
    let renamed = edited_view(&sample, |view| view.name = "Renamed".to_owned());
    let inverse = sample
        .workspace
        .apply_database_operation(&Operation::PutView(renamed))
        .unwrap();
    assert_eq!(sample.workspace.view(sample.view).unwrap().name, "Renamed");
    sample.workspace.apply_database_operation(&inverse).unwrap();
    assert_eq!(sample.workspace, before);
}

#[test]
fn put_view_rejects_a_block_that_is_not_a_database_view() {
    let mut sample = Sample::new();
    let missing = BlockId::new();
    let orphan = view(ViewId::new(), missing, Vec::new());
    assert_rejected(
        &mut sample.workspace,
        Operation::PutView(orphan),
        DomainError::UnknownBlock(missing),
    );
    let on_page = view(ViewId::new(), sample.page, Vec::new());
    assert_rejected(
        &mut sample.workspace,
        Operation::PutView(on_page),
        DomainError::InvalidParent,
    );
}

#[test]
fn put_view_rejects_unknown_properties_everywhere() {
    let unknown = property_id(99);
    let changes: [fn(&mut View, PropertyId); 5] = [
        |view, property| {
            view.filters.push(Filter {
                property,
                condition: Condition::Checkbox(true),
            });
        },
        |view, property| {
            view.sorts.push(Sort {
                property,
                direction: SortDirection::Descending,
            });
        },
        |view, property| view.visible.push(property),
        |view, property| {
            view.column_widths.insert(property, 50);
        },
        |view, property| view.kind = ViewKind::Board { group_by: property },
    ];
    for change in changes {
        let mut sample = Sample::new();
        let edited = edited_view(&sample, |view| change(view, unknown));
        assert_rejected(
            &mut sample.workspace,
            Operation::PutView(edited),
            DomainError::UnknownProperty(unknown),
        );
    }
}

#[test]
fn put_view_rejects_a_condition_of_the_wrong_kind() {
    let mut sample = Sample::new();
    let cases = [
        (sample.amount, Condition::Text(TextCondition::IsEmpty)),
        (sample.notes, Condition::Number(NumberCondition::Eq(1.0))),
        (sample.tags, Condition::Select(SelectCondition::IsEmpty)),
        (
            sample.status,
            Condition::MultiSelect(MultiSelectCondition::IsEmpty),
        ),
        (sample.done, Condition::Text(TextCondition::IsEmpty)),
        (sample.due, Condition::Checkbox(true)),
    ];
    for (property, condition) in cases {
        let edited = edited_view(&sample, |view| {
            view.filters.push(Filter {
                property,
                condition,
            });
        });
        assert_rejected(
            &mut sample.workspace,
            Operation::PutView(edited),
            DomainError::ConditionKindMismatch,
        );
    }
}

#[test]
fn put_view_accepts_text_conditions_on_title_text_and_url() {
    let mut sample = Sample::new();
    for property in [sample.title, sample.notes, sample.link] {
        let edited = edited_view(&sample, |view| {
            view.filters.push(Filter {
                property,
                condition: Condition::Text(TextCondition::Contains("a".to_owned())),
            });
        });
        sample
            .workspace
            .apply_database_operation(&Operation::PutView(edited))
            .unwrap();
    }
}

#[test]
fn put_view_requires_a_select_property_for_a_board() {
    let mut sample = Sample::new();
    let edited = edited_view(&sample, |view| {
        view.kind = ViewKind::Board {
            group_by: sample.tags,
        };
    });
    assert_rejected(
        &mut sample.workspace,
        Operation::PutView(edited),
        DomainError::ValueKindMismatch,
    );
    let edited = edited_view(&sample, |view| {
        view.kind = ViewKind::Board {
            group_by: sample.status,
        };
    });
    sample
        .workspace
        .apply_database_operation(&Operation::PutView(edited))
        .unwrap();
}

#[test]
fn remove_view_rejects_an_unknown_view_and_returns_the_old_one() {
    let mut sample = Sample::new();
    let unknown = ViewId::new();
    assert_rejected(
        &mut sample.workspace,
        Operation::RemoveView { view: unknown },
        DomainError::UnknownView(unknown),
    );
    let before = sample.workspace.clone();
    let inverse = sample
        .workspace
        .apply_database_operation(&Operation::RemoveView { view: sample.view })
        .unwrap();
    assert!(sample.workspace.view(sample.view).is_none());
    sample.workspace.apply_database_operation(&inverse).unwrap();
    assert_eq!(sample.workspace, before);
}

fn snapshot_of(sample: &Sample) -> WorkspaceSnapshot {
    sample.workspace.to_snapshot()
}

#[test]
fn a_valid_snapshot_is_accepted() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    sample.set(row, sample.amount, PropertyValue::Number(1.0));
    sample.set(
        row,
        sample.tags,
        PropertyValue::MultiSelect(vec![sample.red]),
    );
    assert_eq!(
        Workspace::from_snapshot(snapshot_of(&sample)).unwrap(),
        sample.workspace
    );
}

#[test]
fn snapshot_with_value_of_wrong_kind_is_rejected() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    let mut snapshot = snapshot_of(&sample);
    snapshot
        .values
        .push((row, sample.amount, PropertyValue::Text("x".to_owned())));
    assert_eq!(
        Workspace::from_snapshot(snapshot),
        Err(DomainError::ValueKindMismatch)
    );
}

#[test]
fn snapshot_rejects_inconsistent_databases_values_and_views() {
    let mut sample = Sample::new();
    let row = sample.add_row("a");
    let base = snapshot_of(&sample);

    let mut no_title = base.clone();
    no_title.databases[0].schema.properties.remove(0);
    assert_eq!(
        Workspace::from_snapshot(no_title),
        Err(DomainError::TitleIsBlockText)
    );

    let mut title_value = base.clone();
    title_value
        .values
        .push((row, sample.title, PropertyValue::Text("x".to_owned())));
    assert_eq!(
        Workspace::from_snapshot(title_value),
        Err(DomainError::TitleIsBlockText)
    );

    let mut orphan_value = base.clone();
    orphan_value.values.push((
        BlockId::new(),
        sample.notes,
        PropertyValue::Text("x".to_owned()),
    ));
    assert!(matches!(
        Workspace::from_snapshot(orphan_value),
        Err(DomainError::UnknownBlock(_))
    ));

    let mut bad_option = base.clone();
    bad_option
        .values
        .push((row, sample.status, PropertyValue::Select(OptionId::new())));
    assert!(matches!(
        Workspace::from_snapshot(bad_option),
        Err(DomainError::UnknownOption(_))
    ));

    let mut bad_view = base.clone();
    bad_view.views[0].visible.push(property_id(99));
    assert_eq!(
        Workspace::from_snapshot(bad_view),
        Err(DomainError::UnknownProperty(property_id(99)))
    );

    let mut duplicate_property = base;
    duplicate_property.databases[0]
        .schema
        .properties
        .push(property(sample.notes, "Again", PropertyKind::Text));
    assert_eq!(
        Workspace::from_snapshot(duplicate_property),
        Err(DomainError::AlreadyExists)
    );
}

struct Pool {
    databases: Vec<DatabaseId>,
    rows: Vec<BlockId>,
    properties: Vec<PropertyId>,
    options: Vec<OptionId>,
    views: Vec<ViewId>,
    blocks: Vec<BlockId>,
    typed_values: Vec<(PropertyId, PropertyValue)>,
    typed_filters: Vec<Filter>,
    select: PropertyId,
}

fn pool_and_workspace() -> (Pool, Workspace) {
    let mut sample = Sample::new();
    let rows: Vec<BlockId> = (0..3)
        .map(|n| sample.add_row_with_id(BlockId::from_uuid(Uuid::from_u128(200 + n)), "row"))
        .collect();
    sample.set(rows[0], sample.status, PropertyValue::Select(sample.todo));
    sample.set(rows[0], sample.amount, PropertyValue::Number(3.0));
    sample.set(rows[1], sample.notes, PropertyValue::Text("n".to_owned()));
    sample.set(
        rows[1],
        sample.tags,
        PropertyValue::MultiSelect(vec![sample.red]),
    );
    sample.set(rows[2], sample.done, PropertyValue::Checkbox(true));
    let title = sample.title;
    sample.view_mut().visible = vec![title];
    let typed_values = vec![
        (sample.status, PropertyValue::Select(sample.todo)),
        (sample.status, PropertyValue::Select(sample.finished)),
        (sample.amount, PropertyValue::Number(1.5)),
        (sample.amount, PropertyValue::Number(-2.0)),
        (sample.notes, PropertyValue::Text("t".to_owned())),
        (sample.link, PropertyValue::Url("u".to_owned())),
        (
            sample.tags,
            PropertyValue::MultiSelect(vec![sample.red, sample.blue]),
        ),
        (
            sample.due,
            PropertyValue::Date(CalendarDate::new(2026, 10, 23).unwrap()),
        ),
        (sample.done, PropertyValue::Checkbox(false)),
    ];
    let typed_filters = vec![
        Filter {
            property: sample.status,
            condition: Condition::Select(SelectCondition::Is(sample.todo)),
        },
        Filter {
            property: sample.amount,
            condition: Condition::Number(NumberCondition::Gt(1.0)),
        },
        Filter {
            property: sample.notes,
            condition: Condition::Text(TextCondition::Contains("t".to_owned())),
        },
        Filter {
            property: sample.title,
            condition: Condition::Text(TextCondition::IsNotEmpty),
        },
        Filter {
            property: sample.tags,
            condition: Condition::MultiSelect(MultiSelectCondition::IsEmpty),
        },
        Filter {
            property: sample.done,
            condition: Condition::Checkbox(true),
        },
        Filter {
            property: sample.status,
            condition: Condition::Checkbox(true),
        },
    ];
    let pool = Pool {
        databases: vec![sample.database, DatabaseId::from_uuid(Uuid::from_u128(101))],
        rows,
        properties: vec![
            sample.title,
            sample.status,
            sample.amount,
            sample.tags,
            sample.notes,
            sample.link,
            sample.due,
            sample.done,
            property_id(60),
            property_id(61),
        ],
        options: vec![
            sample.todo,
            sample.doing,
            sample.finished,
            sample.red,
            OptionId::from_uuid(Uuid::from_u128(99)),
        ],
        views: vec![
            sample.view,
            ViewId::from_uuid(Uuid::from_u128(301)),
            ViewId::from_uuid(Uuid::from_u128(302)),
        ],
        blocks: vec![
            sample.embed,
            sample.embed,
            sample.embed,
            sample.page,
            BlockId::from_uuid(Uuid::from_u128(98)),
        ],
        typed_values,
        typed_filters,
        select: sample.status,
    };
    (pool, sample.workspace)
}

fn kind_strategy(options: Vec<OptionId>) -> BoxedStrategy<PropertyKind> {
    let chosen =
        proptest::sample::subsequence(options.clone(), 0..=options.len()).prop_map(|ids| {
            ids.into_iter()
                .map(|id| option(id, "o"))
                .collect::<Vec<SelectOption>>()
        });
    prop_oneof![
        1 => Just(PropertyKind::Title),
        2 => Just(PropertyKind::Text),
        2 => Just(PropertyKind::Number { format: NumberFormat::Plain }),
        2 => chosen.clone().prop_map(|options| PropertyKind::Select { options }),
        2 => chosen.prop_map(|options| PropertyKind::MultiSelect { options }),
        1 => Just(PropertyKind::Date),
        1 => Just(PropertyKind::Checkbox),
        1 => Just(PropertyKind::Url),
    ]
    .boxed()
}

fn property_strategy(pool: &Pool) -> BoxedStrategy<Property> {
    (
        proptest::sample::select(pool.properties.clone()),
        "[a-c]{0,2}",
        kind_strategy(pool.options.clone()),
    )
        .prop_map(|(id, name, kind)| Property { id, name, kind })
        .boxed()
}

fn value_strategy(pool: &Pool) -> BoxedStrategy<(PropertyId, Option<PropertyValue>)> {
    let option = proptest::sample::select(pool.options.clone());
    let loose = (
        proptest::sample::select(pool.properties.clone()),
        prop_oneof![
            2 => Just(None),
            1 => "[a-c]{0,3}".prop_map(|text| Some(PropertyValue::Text(text))),
            1 => prop_oneof![Just(1.5), Just(f64::NAN)]
                .prop_map(|number| Some(PropertyValue::Number(number))),
            1 => option.clone().prop_map(|id| Some(PropertyValue::Select(id))),
            1 => proptest::collection::vec(option, 0..3)
                .prop_map(|ids| Some(PropertyValue::MultiSelect(ids))),
        ],
    );
    let typed = proptest::sample::select(pool.typed_values.clone())
        .prop_map(|(property, value)| (property, Some(value)));
    let cleared = proptest::sample::select(pool.typed_values.clone())
        .prop_map(|(property, _)| (property, None));
    prop_oneof![6 => typed, 3 => cleared, 2 => loose].boxed()
}

fn view_strategy(pool: &Pool) -> BoxedStrategy<View> {
    let property = proptest::sample::select(pool.properties.clone());
    let filters =
        proptest::collection::vec(proptest::sample::select(pool.typed_filters.clone()), 0..3);
    let sorts = proptest::collection::vec(
        (property.clone(), any::<bool>()).prop_map(|(property, ascending)| Sort {
            property,
            direction: if ascending {
                SortDirection::Ascending
            } else {
                SortDirection::Descending
            },
        }),
        0..2,
    );
    let visible = proptest::sample::subsequence(pool.properties[..8].to_vec(), 0..4);
    let widths = proptest::collection::btree_map(property.clone(), 10u32..300, 0..2);
    let select = pool.select;
    let kind = prop_oneof![
        3 => Just(ViewKind::Table),
        3 => Just(ViewKind::Board { group_by: select }),
        1 => property.prop_map(|group_by| ViewKind::Board { group_by }),
    ];
    let manual = proptest::sample::subsequence(pool.rows.clone(), 0..3);
    (
        proptest::sample::select(pool.views.clone()),
        proptest::sample::select(pool.blocks.clone()),
        kind,
        filters,
        sorts,
        visible,
        widths,
        manual,
    )
        .prop_map(
            |(id, block, kind, filters, sorts, visible, column_widths, manual_order)| View {
                id,
                block,
                name: "v".to_owned(),
                sort_key: key(1),
                kind,
                filters,
                sorts,
                visible,
                column_widths,
                manual_order,
            },
        )
        .boxed()
}

fn operation_strategy(pool: &Pool) -> BoxedStrategy<Operation> {
    let any_database = proptest::sample::select(pool.databases.clone());
    let schema = prop_oneof![
        3 => proptest::sample::select(vec![property_id(60), property_id(61)])
            .prop_map(|id| vec![Property { id, name: "t".to_owned(), kind: PropertyKind::Title }]),
        1 => proptest::collection::vec(property_strategy(pool), 0..3),
    ];
    let icon = proptest::option::of(Just(Emoji::try_from("📋").unwrap()));
    prop_oneof![
        3 => (any_database.clone(), "[a-c]{0,2}")
            .prop_map(|(database, name)| Operation::SetDatabaseName { database, name }),
        1 => (any_database.clone(), icon)
            .prop_map(|(database, icon)| Operation::SetDatabaseIcon { database, icon }),
        1 => (any_database.clone(), any::<bool>())
            .prop_map(|(database, alive)| Operation::SetDatabaseAlive { database, alive }),
        4 => (any_database.clone(), property_strategy(pool), 0usize..9)
            .prop_map(|(database, property, index)| Operation::PutProperty {
                database,
                property,
                index,
            }),
        3 => (any_database.clone(), proptest::sample::select(pool.properties.clone()))
            .prop_map(|(database, property)| Operation::RemoveProperty { database, property }),
        8 => (proptest::sample::select(pool.rows.clone()), value_strategy(pool))
            .prop_map(|(row, (property, value))| Operation::SetValue { row, property, value }),
        4 => view_strategy(pool).prop_map(Operation::PutView),
        2 => proptest::sample::select(pool.views.clone())
            .prop_map(|view| Operation::RemoveView { view }),
        2 => (any_database.clone(), schema).prop_map(|(id, properties)| {
            Operation::InsertDatabase(database(id, "d", properties))
        }),
        2 => any_database.prop_map(|database| Operation::RemoveDatabase { database }),
    ]
    .boxed()
}

proptest! {
    #[test]
    fn apply_then_inverse_restores_workspace(
        operations in {
            let (pool, _) = pool_and_workspace();
            proptest::collection::vec(operation_strategy(&pool), 1..30)
        }
    ) {
        let (_, mut workspace) = pool_and_workspace();
        let initial = workspace.clone();
        let mut inverses = Vec::new();
        for operation in operations {
            let before = workspace.clone();
            match workspace.apply(&Transaction::new(vec![operation])) {
                Ok(inverse) => {
                    prop_assert_eq!(workspace.validate_databases(), Ok(()));
                    let mut probe = workspace.clone();
                    prop_assert!(probe.apply(&inverse).is_ok());
                    prop_assert_eq!(&probe, &before);
                    inverses.push(inverse);
                }
                Err(_) => prop_assert_eq!(&workspace, &before),
            }
        }
        for inverse in inverses.iter().rev() {
            prop_assert!(workspace.apply(inverse).is_ok());
        }
        prop_assert_eq!(workspace, initial);
    }
}
