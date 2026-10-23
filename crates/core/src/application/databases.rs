use std::collections::BTreeMap;

use crate::domain::{
    Block, BlockId, BlockKind, Database, DatabaseId, DomainError, Operation, OptionColor, OptionId,
    Parent, Property, PropertyId, PropertyKind, PropertyValue, RichText, Schema, SelectOption,
    SortKey, Timestamp, Transaction, View, ViewId, ViewKind, Workspace,
};

use super::{Built, Command, Created, blocks};

pub(crate) fn build(
    workspace: &Workspace,
    command: &Command,
    now: Timestamp,
) -> Result<Built, DomainError> {
    match command {
        Command::CreateDatabase => Ok(create_database(workspace, now)),
        Command::CreateDatabaseWithView { after } => {
            create_database_with_view(workspace, *after, now)
        }
        Command::InsertDatabaseView { after, database } => {
            insert_database_view(workspace, *after, *database, now)
        }
        Command::RenameDatabase { database, name } => {
            known_database(workspace, *database)?;
            Ok(single(Operation::SetDatabaseName {
                database: *database,
                name: name.clone(),
            }))
        }
        Command::SetDatabaseIcon { database, icon } => {
            known_database(workspace, *database)?;
            Ok(single(Operation::SetDatabaseIcon {
                database: *database,
                icon: icon.clone(),
            }))
        }
        Command::DeleteDatabase { database } => delete_database(workspace, *database),
        Command::AddProperty {
            database,
            name,
            kind,
        } => add_property(workspace, *database, name, kind),
        Command::RenameProperty {
            database,
            property,
            name,
        } => rename_property(workspace, *database, *property, name),
        Command::ChangePropertyKind {
            database,
            property,
            kind,
        } => change_property_kind(workspace, *database, *property, kind),
        Command::RemoveProperty { database, property } => {
            remove_property(workspace, *database, *property)
        }
        Command::MoveProperty {
            database,
            property,
            index,
        } => move_property(workspace, *database, *property, *index),
        Command::AddSelectOption {
            database,
            property,
            name,
            color,
        } => add_select_option(workspace, *database, *property, name, *color),
        Command::RenameSelectOption {
            database,
            property,
            option,
            name,
        } => rename_select_option(workspace, *database, *property, *option, name),
        Command::RemoveSelectOption {
            database,
            property,
            option,
        } => remove_select_option(workspace, *database, *property, *option),
        Command::AddRow { database, values } => add_row(workspace, *database, values, now),
        Command::SetValue {
            row,
            property,
            value,
        } => {
            live_block(workspace, *row)?;
            Ok(single(Operation::SetValue {
                row: *row,
                property: *property,
                value: value.clone(),
            }))
        }
        Command::AddView { block, name, kind } => add_view(workspace, *block, name, *kind),
        Command::RenameView { view, name } => edit_view(workspace, *view, |edited| {
            edited.name.clone_from(name);
        }),
        Command::RemoveView { view } => remove_view(workspace, *view),
        Command::SetFilters { view, filters } => edit_view(workspace, *view, |edited| {
            edited.filters.clone_from(filters);
        }),
        Command::SetSorts { view, sorts } => edit_view(workspace, *view, |edited| {
            edited.sorts.clone_from(sorts);
        }),
        Command::SetVisibleProperties { view, properties } => {
            edit_view(workspace, *view, |edited| {
                edited.visible.clone_from(properties);
            })
        }
        Command::SetColumnWidth {
            view,
            property,
            width,
        } => edit_view(workspace, *view, |edited| {
            edited.column_widths.insert(*property, *width);
        }),
        Command::SetViewKind { view, kind } => edit_view(workspace, *view, |edited| {
            edited.kind = *kind;
        }),
        Command::MoveRow { view, row, before } => move_row(workspace, *view, *row, *before),
        Command::MoveCard {
            view,
            row,
            group,
            before,
        } => move_card(workspace, *view, *row, *group, *before),
        Command::CreatePage { .. }
        | Command::InsertBlock { .. }
        | Command::InsertBlockAt { .. }
        | Command::EditText { .. }
        | Command::SplitBlock { .. }
        | Command::MergeWithPrevious { .. }
        | Command::DeleteRange { .. }
        | Command::SetMarks { .. }
        | Command::Indent { .. }
        | Command::Outdent { .. }
        | Command::ChangeKind { .. }
        | Command::MoveBlock { .. }
        | Command::DeleteBlocks { .. }
        | Command::SetPageIcon { .. } => blocks::build(workspace, command, now),
    }
}

fn single(operation: Operation) -> Built {
    operations(vec![operation])
}

fn operations(operations: Vec<Operation>) -> Built {
    Built {
        transaction: Transaction::new(operations),
        ..Built::default()
    }
}

fn operations_creating(operations: Vec<Operation>, created: Vec<Created>) -> Built {
    Built {
        transaction: Transaction::new(operations),
        created,
        caret: None,
    }
}

fn known_database(workspace: &Workspace, id: DatabaseId) -> Result<&Database, DomainError> {
    workspace
        .database(id)
        .filter(|database| database.alive)
        .ok_or(DomainError::UnknownDatabase(id))
}

fn live_block(workspace: &Workspace, id: BlockId) -> Result<&Block, DomainError> {
    workspace
        .block(id)
        .filter(|block| block.alive)
        .ok_or(DomainError::UnknownBlock(id))
}

fn locate(
    workspace: &Workspace,
    database: DatabaseId,
    property: PropertyId,
) -> Result<(usize, &Property), DomainError> {
    known_database(workspace, database)?
        .schema
        .find(property)
        .ok_or(DomainError::UnknownProperty(property))
}

fn property_ids(database: &Database) -> Vec<PropertyId> {
    database
        .schema
        .properties
        .iter()
        .map(|property| property.id)
        .collect()
}

fn new_view(
    block: BlockId,
    name: &str,
    kind: ViewKind,
    sort_key: SortKey,
    visible: Vec<PropertyId>,
) -> View {
    View {
        id: ViewId::new(),
        block,
        name: name.to_owned(),
        sort_key,
        kind,
        filters: Vec::new(),
        sorts: Vec::new(),
        visible,
        column_widths: BTreeMap::new(),
        manual_order: Vec::new(),
    }
}

fn new_database(workspace: &Workspace, now: Timestamp) -> (Database, PropertyId) {
    let title = Property {
        id: PropertyId::new(),
        name: "Name".to_owned(),
        kind: PropertyKind::Title,
    };
    let title_id = title.id;
    let last = workspace
        .databases
        .values()
        .map(|database| &database.sort_key)
        .max();
    let database = Database {
        id: DatabaseId::new(),
        name: "Untitled".to_owned(),
        icon: None,
        schema: Schema {
            properties: vec![title],
        },
        sort_key: SortKey::between(last, None),
        alive: true,
        created_at: now,
    };
    (database, title_id)
}

fn database_view_block(
    workspace: &Workspace,
    after: BlockId,
    database: DatabaseId,
    now: Timestamp,
) -> Result<Block, DomainError> {
    let anchor = live_block(workspace, after)?;
    let upper = workspace
        .children
        .get(&anchor.parent)
        .into_iter()
        .flatten()
        .find(|(key, _)| *key > anchor.sort_key)
        .map(|(key, _)| key);
    Ok(Block {
        id: BlockId::new(),
        kind: BlockKind::DatabaseView { database },
        parent: anchor.parent,
        sort_key: SortKey::between(Some(&anchor.sort_key), upper),
        text: RichText::default(),
        alive: true,
        created_at: now,
    })
}

fn create_database(workspace: &Workspace, now: Timestamp) -> Built {
    let (database, _) = new_database(workspace, now);
    let created = vec![Created::Database(database.id)];
    operations_creating(vec![Operation::InsertDatabase(database)], created)
}

fn create_database_with_view(
    workspace: &Workspace,
    after: BlockId,
    now: Timestamp,
) -> Result<Built, DomainError> {
    let (database, title) = new_database(workspace, now);
    let block = database_view_block(workspace, after, database.id, now)?;
    let view = new_view(
        block.id,
        "Table",
        ViewKind::Table,
        SortKey::between(None, None),
        vec![title],
    );
    let created = vec![
        Created::Database(database.id),
        Created::Block(block.id),
        Created::View(view.id),
    ];
    Ok(operations_creating(
        vec![
            Operation::InsertDatabase(database),
            Operation::InsertBlock(block),
            Operation::PutView(view),
        ],
        created,
    ))
}

fn insert_database_view(
    workspace: &Workspace,
    after: BlockId,
    id: DatabaseId,
    now: Timestamp,
) -> Result<Built, DomainError> {
    let database = known_database(workspace, id)?;
    let block = database_view_block(workspace, after, id, now)?;
    let view = new_view(
        block.id,
        "Table",
        ViewKind::Table,
        SortKey::between(None, None),
        property_ids(database),
    );
    let created = vec![Created::Block(block.id), Created::View(view.id)];
    Ok(operations_creating(
        vec![Operation::InsertBlock(block), Operation::PutView(view)],
        created,
    ))
}

fn delete_database(workspace: &Workspace, database: DatabaseId) -> Result<Built, DomainError> {
    let existing = workspace
        .database(database)
        .ok_or(DomainError::UnknownDatabase(database))?;
    if !existing.alive {
        return Ok(Built::default());
    }
    let mut operations = vec![Operation::SetDatabaseAlive {
        database,
        alive: false,
    }];
    operations.extend(
        workspace
            .rows(database)
            .map(|row| Operation::SetBlockAlive {
                block: row.id,
                alive: false,
            }),
    );
    Ok(self::operations(operations))
}

fn stored_values(workspace: &Workspace, property: PropertyId) -> Vec<(BlockId, &PropertyValue)> {
    let mut stored: Vec<(BlockId, &PropertyValue)> = workspace
        .values
        .iter()
        .filter(|((_, id), _)| *id == property)
        .map(|((row, _), value)| (*row, value))
        .collect();
    stored.sort_by_key(|(row, _)| *row);
    stored
}

fn set_value(row: BlockId, property: PropertyId, value: Option<PropertyValue>) -> Operation {
    Operation::SetValue {
        row,
        property,
        value,
    }
}

fn clear_values(
    property: PropertyId,
    stored: &[(BlockId, &PropertyValue)],
) -> impl Iterator<Item = Operation> {
    stored
        .iter()
        .map(move |(row, _)| set_value(*row, property, None))
}

fn adjusted_views(
    workspace: &Workspace,
    database: DatabaseId,
    adjust: impl Fn(&mut View),
) -> Vec<Operation> {
    workspace
        .database_views(database)
        .into_iter()
        .filter_map(|view| {
            let mut adjusted = view.clone();
            adjust(&mut adjusted);
            (adjusted != *view).then_some(Operation::PutView(adjusted))
        })
        .collect()
}

fn add_property(
    workspace: &Workspace,
    id: DatabaseId,
    name: &str,
    kind: &PropertyKind,
) -> Result<Built, DomainError> {
    let database = known_database(workspace, id)?;
    if *kind == PropertyKind::Title {
        return Err(DomainError::TitleIsBlockText);
    }
    let property = Property {
        id: PropertyId::new(),
        name: name.to_owned(),
        kind: kind.clone(),
    };
    let added = property.id;
    let mut operations = vec![Operation::PutProperty {
        database: id,
        property,
        index: database.schema.properties.len(),
    }];
    operations.extend(adjusted_views(workspace, id, |view| {
        view.visible.push(added);
    }));
    Ok(self::operations(operations))
}

fn rename_property(
    workspace: &Workspace,
    database: DatabaseId,
    property: PropertyId,
    name: &str,
) -> Result<Built, DomainError> {
    let (index, existing) = locate(workspace, database, property)?;
    Ok(single(Operation::PutProperty {
        database,
        property: Property {
            id: property,
            name: name.to_owned(),
            kind: existing.kind.clone(),
        },
        index,
    }))
}

fn move_property(
    workspace: &Workspace,
    database: DatabaseId,
    property: PropertyId,
    index: usize,
) -> Result<Built, DomainError> {
    let (_, existing) = locate(workspace, database, property)?;
    Ok(single(Operation::PutProperty {
        database,
        property: existing.clone(),
        index,
    }))
}

fn change_property_kind(
    workspace: &Workspace,
    database: DatabaseId,
    property: PropertyId,
    kind: &PropertyKind,
) -> Result<Built, DomainError> {
    let (index, existing) = locate(workspace, database, property)?;
    if existing.kind == PropertyKind::Title || *kind == PropertyKind::Title {
        return Err(DomainError::TitleIsBlockText);
    }
    let stored = stored_values(workspace, property);
    let converted = convert(&existing.kind, kind, &stored);
    let mut operations: Vec<Operation> = clear_values(property, &stored).collect();
    operations.extend(adjusted_views(workspace, database, |view| {
        view.filters
            .retain(|filter| filter.property != property || filter.condition.fits(&converted.kind));
        let grouped = matches!(view.kind, ViewKind::Board { group_by } if group_by == property);
        if grouped && !matches!(converted.kind, PropertyKind::Select { .. }) {
            view.kind = ViewKind::Table;
        }
    }));
    operations.push(Operation::PutProperty {
        database,
        property: Property {
            id: property,
            name: existing.name.clone(),
            kind: converted.kind,
        },
        index,
    });
    operations.extend(
        converted
            .values
            .into_iter()
            .map(|(row, value)| set_value(row, property, Some(value))),
    );
    Ok(self::operations(operations))
}

fn remove_property(
    workspace: &Workspace,
    database: DatabaseId,
    property: PropertyId,
) -> Result<Built, DomainError> {
    let (_, existing) = locate(workspace, database, property)?;
    if existing.kind == PropertyKind::Title {
        return Err(DomainError::TitleIsBlockText);
    }
    let stored = stored_values(workspace, property);
    let mut operations: Vec<Operation> = clear_values(property, &stored).collect();
    operations.extend(adjusted_views(workspace, database, |view| {
        view.filters.retain(|filter| filter.property != property);
        view.sorts.retain(|sort| sort.property != property);
        view.visible.retain(|visible| *visible != property);
        view.column_widths.remove(&property);
        if matches!(view.kind, ViewKind::Board { group_by } if group_by == property) {
            view.kind = ViewKind::Table;
        }
    }));
    operations.push(Operation::RemoveProperty { database, property });
    Ok(self::operations(operations))
}

fn with_options(
    kind: &PropertyKind,
    change: impl FnOnce(&[SelectOption]) -> Result<Vec<SelectOption>, DomainError>,
) -> Result<PropertyKind, DomainError> {
    match kind {
        PropertyKind::Select { options } => Ok(PropertyKind::Select {
            options: change(options)?,
        }),
        PropertyKind::MultiSelect { options } => Ok(PropertyKind::MultiSelect {
            options: change(options)?,
        }),
        _ => Err(DomainError::ValueKindMismatch),
    }
}

fn require_option(options: &[SelectOption], option: OptionId) -> Result<(), DomainError> {
    if options.iter().any(|candidate| candidate.id == option) {
        Ok(())
    } else {
        Err(DomainError::UnknownOption(option))
    }
}

fn put_kind(
    database: DatabaseId,
    index: usize,
    existing: &Property,
    kind: PropertyKind,
) -> Operation {
    Operation::PutProperty {
        database,
        property: Property {
            id: existing.id,
            name: existing.name.clone(),
            kind,
        },
        index,
    }
}

fn add_select_option(
    workspace: &Workspace,
    database: DatabaseId,
    property: PropertyId,
    name: &str,
    color: OptionColor,
) -> Result<Built, DomainError> {
    let (index, existing) = locate(workspace, database, property)?;
    let kind = with_options(&existing.kind, |options| {
        let mut extended = options.to_vec();
        extended.push(SelectOption {
            id: OptionId::new(),
            name: name.to_owned(),
            color,
        });
        Ok(extended)
    })?;
    Ok(single(put_kind(database, index, existing, kind)))
}

fn rename_select_option(
    workspace: &Workspace,
    database: DatabaseId,
    property: PropertyId,
    option: OptionId,
    name: &str,
) -> Result<Built, DomainError> {
    let (index, existing) = locate(workspace, database, property)?;
    let kind = with_options(&existing.kind, |options| {
        require_option(options, option)?;
        Ok(options
            .iter()
            .map(|candidate| {
                let mut renamed = candidate.clone();
                if renamed.id == option {
                    renamed.name = name.to_owned();
                }
                renamed
            })
            .collect())
    })?;
    Ok(single(put_kind(database, index, existing, kind)))
}

fn remove_select_option(
    workspace: &Workspace,
    database: DatabaseId,
    property: PropertyId,
    option: OptionId,
) -> Result<Built, DomainError> {
    let (index, existing) = locate(workspace, database, property)?;
    let kind = with_options(&existing.kind, |options| {
        require_option(options, option)?;
        Ok(options
            .iter()
            .filter(|candidate| candidate.id != option)
            .cloned()
            .collect())
    })?;
    let mut operations: Vec<Operation> = stored_values(workspace, property)
        .into_iter()
        .filter_map(|(row, value)| match value {
            PropertyValue::Select(chosen) if *chosen == option => {
                Some(set_value(row, property, None))
            }
            PropertyValue::MultiSelect(chosen) if chosen.contains(&option) => {
                let kept: Vec<OptionId> = chosen
                    .iter()
                    .copied()
                    .filter(|candidate| *candidate != option)
                    .collect();
                let value = (!kept.is_empty()).then_some(PropertyValue::MultiSelect(kept));
                Some(set_value(row, property, value))
            }
            _ => None,
        })
        .collect();
    operations.extend(adjusted_views(workspace, database, |view| {
        view.filters.retain(|filter| {
            filter.property != property || !filter.condition.mentions_option(option)
        });
    }));
    operations.push(put_kind(database, index, existing, kind));
    Ok(self::operations(operations))
}

fn add_row(
    workspace: &Workspace,
    database: DatabaseId,
    values: &[(PropertyId, PropertyValue)],
    now: Timestamp,
) -> Result<Built, DomainError> {
    known_database(workspace, database)?;
    let parent = Parent::Database(database);
    let last = workspace
        .children
        .get(&parent)
        .and_then(|rows| rows.iter().next_back())
        .map(|(key, _)| key);
    let row = Block {
        id: BlockId::new(),
        kind: BlockKind::Page { icon: None },
        parent,
        sort_key: SortKey::between(last, None),
        text: RichText::default(),
        alive: true,
        created_at: now,
    };
    let id = row.id;
    let mut operations = vec![Operation::InsertBlock(row)];
    operations.extend(
        values
            .iter()
            .map(|(property, value)| set_value(id, *property, Some(value.clone()))),
    );
    Ok(operations_creating(operations, vec![Created::Block(id)]))
}

fn add_view(
    workspace: &Workspace,
    block: BlockId,
    name: &str,
    kind: ViewKind,
) -> Result<Built, DomainError> {
    let owner = live_block(workspace, block)?;
    let BlockKind::DatabaseView { database: id } = owner.kind else {
        return Err(DomainError::InvalidParent);
    };
    let database = known_database(workspace, id)?;
    let last = workspace.views(block).last().map(|view| &view.sort_key);
    let view = new_view(
        block,
        name,
        kind,
        SortKey::between(last, None),
        property_ids(database),
    );
    let created = vec![Created::View(view.id)];
    Ok(operations_creating(vec![Operation::PutView(view)], created))
}

fn known_view(workspace: &Workspace, id: ViewId) -> Result<&View, DomainError> {
    workspace
        .view(id)
        .filter(|view| live_block(workspace, view.block).is_ok())
        .ok_or(DomainError::UnknownView(id))
}

fn edit_view(
    workspace: &Workspace,
    id: ViewId,
    change: impl FnOnce(&mut View),
) -> Result<Built, DomainError> {
    let mut edited = known_view(workspace, id)?.clone();
    change(&mut edited);
    Ok(single(Operation::PutView(edited)))
}

fn remove_view(workspace: &Workspace, id: ViewId) -> Result<Built, DomainError> {
    let view = known_view(workspace, id)?;
    if workspace.views(view.block).count() <= 1 {
        return Err(DomainError::LastView);
    }
    Ok(single(Operation::RemoveView { view: id }))
}

fn reorder(
    mut order: Vec<BlockId>,
    row: BlockId,
    before: Option<BlockId>,
    group: Option<&[BlockId]>,
) -> Result<Vec<BlockId>, DomainError> {
    if !order.contains(&row) {
        return Err(DomainError::UnknownBlock(row));
    }
    if before == Some(row) {
        return Ok(order);
    }
    order.retain(|id| *id != row);
    let at = match (before, group) {
        (Some(anchor), _) => order
            .iter()
            .position(|id| *id == anchor)
            .ok_or(DomainError::UnknownBlock(anchor))?,
        (None, Some(group)) => order
            .iter()
            .rposition(|id| group.contains(id))
            .map_or(order.len(), |last| last + 1),
        (None, None) => order.len(),
    };
    order.insert(at, row);
    Ok(order)
}

fn move_row(
    workspace: &Workspace,
    id: ViewId,
    row: BlockId,
    before: Option<BlockId>,
) -> Result<Built, DomainError> {
    let view = known_view(workspace, id)?;
    if !view.sorts.is_empty() {
        return Err(DomainError::ManualOrderWithSorts);
    }
    let mut moved = view.clone();
    moved.manual_order = reorder(workspace.query(id)?, row, before, None)?;
    Ok(single(Operation::PutView(moved)))
}

fn move_card(
    workspace: &Workspace,
    id: ViewId,
    row: BlockId,
    group: Option<OptionId>,
    before: Option<BlockId>,
) -> Result<Built, DomainError> {
    let view = known_view(workspace, id)?;
    let ViewKind::Board { group_by } = view.kind else {
        return Err(DomainError::ValueKindMismatch);
    };
    let groups = workspace.board_groups(id)?;
    let current = groups
        .iter()
        .find(|candidate| candidate.rows.contains(&row))
        .ok_or(DomainError::UnknownBlock(row))?;
    let target = groups
        .iter()
        .find(|candidate| candidate.option == group)
        .ok_or_else(|| group.map_or(DomainError::ValueKindMismatch, DomainError::UnknownOption))?;
    let changed = current.option != group;
    let mut operations = Vec::new();
    if changed {
        operations.push(set_value(row, group_by, group.map(PropertyValue::Select)));
    }
    if view.sorts.is_empty() {
        if let Some(anchor) = before.filter(|anchor| *anchor != row)
            && !target.rows.contains(&anchor)
        {
            return Err(DomainError::UnknownBlock(anchor));
        }
        let mut moved = view.clone();
        moved.manual_order = reorder(workspace.query(id)?, row, before, Some(&target.rows))?;
        operations.push(Operation::PutView(moved));
    } else if !changed {
        return Err(DomainError::ManualOrderWithSorts);
    }
    Ok(self::operations(operations))
}

struct Converted {
    kind: PropertyKind,
    values: Vec<(BlockId, PropertyValue)>,
}

fn convert(
    old: &PropertyKind,
    target: &PropertyKind,
    stored: &[(BlockId, &PropertyValue)],
) -> Converted {
    let values = match target {
        PropertyKind::Text => stringified(old, stored, PropertyValue::Text),
        PropertyKind::Url => stringified(old, stored, PropertyValue::Url),
        PropertyKind::Number { .. } => stored
            .iter()
            .filter_map(|(row, value)| {
                number_of(value).map(|number| (*row, PropertyValue::Number(number)))
            })
            .collect(),
        PropertyKind::Date => kept(stored, |value| matches!(value, PropertyValue::Date(_))),
        PropertyKind::Checkbox => kept(stored, |value| matches!(value, PropertyValue::Checkbox(_))),
        PropertyKind::Select { options } => return into_select(old, options, stored),
        PropertyKind::MultiSelect { options } => return into_multi_select(old, options, stored),
        PropertyKind::Title => Vec::new(),
    };
    Converted {
        kind: target.clone(),
        values,
    }
}

fn kept(
    stored: &[(BlockId, &PropertyValue)],
    accepts: impl Fn(&PropertyValue) -> bool,
) -> Vec<(BlockId, PropertyValue)> {
    stored
        .iter()
        .filter(|(_, value)| accepts(value))
        .map(|(row, value)| (*row, (*value).clone()))
        .collect()
}

fn stringified(
    old: &PropertyKind,
    stored: &[(BlockId, &PropertyValue)],
    wrap: fn(String) -> PropertyValue,
) -> Vec<(BlockId, PropertyValue)> {
    let options = old.options().unwrap_or_default();
    stored
        .iter()
        .filter_map(|(row, value)| {
            text_of(options, value)
                .filter(|text| !text.is_empty())
                .map(|text| (*row, wrap(text)))
        })
        .collect()
}

fn text_of(options: &[SelectOption], value: &PropertyValue) -> Option<String> {
    let name_of = |id: &OptionId| {
        options
            .iter()
            .find(|option| option.id == *id)
            .map(|option| option.name.as_str())
    };
    match value {
        PropertyValue::Text(text) | PropertyValue::Url(text) => Some(text.clone()),
        PropertyValue::Number(number) => Some(format!("{number}")),
        PropertyValue::Select(id) => name_of(id).map(str::to_owned),
        PropertyValue::MultiSelect(ids) => {
            let names: Vec<&str> = ids.iter().filter_map(name_of).collect();
            Some(names.join(", "))
        }
        PropertyValue::Date(date) => Some(format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            date.month(),
            date.day()
        )),
        PropertyValue::Checkbox(true) => Some("true".to_owned()),
        PropertyValue::Checkbox(false) => None,
    }
}

fn number_of(value: &PropertyValue) -> Option<f64> {
    match value {
        PropertyValue::Text(text) | PropertyValue::Url(text) => text
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite()),
        PropertyValue::Number(number) => Some(*number),
        _ => None,
    }
}

fn inherited_options(given: &[SelectOption], old: &PropertyKind) -> Vec<SelectOption> {
    if given.is_empty() {
        old.options().unwrap_or_default().to_vec()
    } else {
        given.to_vec()
    }
}

fn chosen_options(value: &PropertyValue) -> Option<Vec<OptionId>> {
    match value {
        PropertyValue::Select(option) => Some(vec![*option]),
        PropertyValue::MultiSelect(options) => Some(options.clone()),
        _ => None,
    }
}

fn into_select(
    old: &PropertyKind,
    given: &[SelectOption],
    stored: &[(BlockId, &PropertyValue)],
) -> Converted {
    if matches!(old, PropertyKind::Text | PropertyKind::Url) {
        let mut options = given.to_vec();
        let mut values = Vec::new();
        for (row, value) in stored {
            let (PropertyValue::Text(text) | PropertyValue::Url(text)) = value else {
                continue;
            };
            let name = text.trim();
            if name.is_empty() {
                continue;
            }
            let id = match options.iter().find(|option| option.name == name) {
                Some(option) => option.id,
                None => {
                    let option = SelectOption {
                        id: OptionId::new(),
                        name: name.to_owned(),
                        color: OptionColor::Default,
                    };
                    let id = option.id;
                    options.push(option);
                    id
                }
            };
            values.push((*row, PropertyValue::Select(id)));
        }
        return Converted {
            kind: PropertyKind::Select { options },
            values,
        };
    }
    let options = inherited_options(given, old);
    let values = stored
        .iter()
        .filter_map(|(row, value)| {
            let first = chosen_options(value)?.first().copied()?;
            options
                .iter()
                .any(|option| option.id == first)
                .then_some((*row, PropertyValue::Select(first)))
        })
        .collect();
    Converted {
        kind: PropertyKind::Select { options },
        values,
    }
}

fn into_multi_select(
    old: &PropertyKind,
    given: &[SelectOption],
    stored: &[(BlockId, &PropertyValue)],
) -> Converted {
    let options = inherited_options(given, old);
    let values = stored
        .iter()
        .filter_map(|(row, value)| {
            let chosen: Vec<OptionId> = chosen_options(value)?
                .into_iter()
                .filter(|id| options.iter().any(|option| option.id == *id))
                .collect();
            (!chosen.is_empty()).then_some((*row, PropertyValue::MultiSelect(chosen)))
        })
        .collect();
    Converted {
        kind: PropertyKind::MultiSelect { options },
        values,
    }
}

#[cfg(test)]
mod fuzz;
#[cfg(test)]
mod tests;
