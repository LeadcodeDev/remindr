use std::collections::BTreeMap;

use crate::domain::{
    Block, BlockId, BlockKind, Database, DatabaseId, NumberFormat, Operation, OptionColor,
    OptionId, Parent, Property, PropertyId, PropertyKind, PropertyValue, RichText, Schema,
    SelectOption, SortKey, Timestamp, Transaction, Uuid, View, ViewId, ViewKind, Workspace,
    WorkspaceSnapshot,
};

const DIGITS: &str = "123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

pub fn key(position: usize) -> SortKey {
    SortKey::parse(&DIGITS[position..=position]).unwrap()
}

pub fn property_id(n: u128) -> PropertyId {
    PropertyId::from_uuid(Uuid::from_u128(n))
}

pub fn option_id(n: u128) -> OptionId {
    OptionId::from_uuid(Uuid::from_u128(n))
}

pub fn option(id: OptionId, name: &str) -> SelectOption {
    SelectOption {
        id,
        name: name.to_owned(),
        color: OptionColor::Default,
    }
}

pub fn property(id: PropertyId, name: &str, kind: PropertyKind) -> Property {
    Property {
        id,
        name: name.to_owned(),
        kind,
    }
}

pub fn block(id: BlockId, kind: BlockKind, parent: Parent, sort_key: SortKey) -> Block {
    Block {
        id,
        kind,
        parent,
        sort_key,
        text: RichText::default(),
        alive: true,
        created_at: Timestamp::from_millis(0),
    }
}

pub fn database(id: DatabaseId, name: &str, properties: Vec<Property>) -> Database {
    Database {
        id,
        name: name.to_owned(),
        icon: None,
        schema: Schema { properties },
        sort_key: key(0),
        alive: true,
        created_at: Timestamp::from_millis(0),
    }
}

pub fn view(id: ViewId, block: BlockId, visible: Vec<PropertyId>) -> View {
    View {
        id,
        block,
        name: "Table".to_owned(),
        sort_key: key(0),
        kind: ViewKind::Table,
        filters: Vec::new(),
        sorts: Vec::new(),
        visible,
        column_widths: BTreeMap::new(),
        manual_order: Vec::new(),
    }
}

pub struct Sample {
    pub workspace: Workspace,
    pub database: DatabaseId,
    pub page: BlockId,
    pub embed: BlockId,
    pub view: ViewId,
    pub title: PropertyId,
    pub status: PropertyId,
    pub amount: PropertyId,
    pub tags: PropertyId,
    pub due: PropertyId,
    pub done: PropertyId,
    pub link: PropertyId,
    pub notes: PropertyId,
    pub todo: OptionId,
    pub doing: OptionId,
    pub finished: OptionId,
    pub red: OptionId,
    pub blue: OptionId,
    rows: usize,
}

impl Sample {
    pub fn new() -> Sample {
        let database = DatabaseId::from_uuid(Uuid::from_u128(100));
        let page = BlockId::from_uuid(Uuid::from_u128(1));
        let embed = BlockId::from_uuid(Uuid::from_u128(2));
        let view_id = ViewId::from_uuid(Uuid::from_u128(300));
        let todo = option_id(20);
        let doing = option_id(21);
        let finished = option_id(22);
        let red = option_id(23);
        let blue = option_id(24);
        let properties = vec![
            property(property_id(10), "Name", PropertyKind::Title),
            property(
                property_id(11),
                "Status",
                PropertyKind::Select {
                    options: vec![
                        option(todo, "Todo"),
                        option(doing, "Doing"),
                        option(finished, "Done"),
                    ],
                },
            ),
            property(
                property_id(12),
                "Amount",
                PropertyKind::Number {
                    format: NumberFormat::Plain,
                },
            ),
            property(
                property_id(13),
                "Tags",
                PropertyKind::MultiSelect {
                    options: vec![option(red, "Red"), option(blue, "Blue")],
                },
            ),
            property(property_id(14), "Due", PropertyKind::Date),
            property(property_id(15), "Done", PropertyKind::Checkbox),
            property(property_id(16), "Link", PropertyKind::Url),
            property(property_id(17), "Notes", PropertyKind::Text),
        ];
        let visible = properties.iter().map(|property| property.id).collect();
        let snapshot = WorkspaceSnapshot {
            blocks: vec![
                block(page, BlockKind::Page { icon: None }, Parent::Root, key(0)),
                block(
                    embed,
                    BlockKind::DatabaseView { database },
                    Parent::Block(page),
                    key(0),
                ),
            ],
            databases: vec![self::database(database, "Tasks", properties)],
            values: Vec::new(),
            views: vec![view(view_id, embed, visible)],
        };
        Sample {
            workspace: Workspace::from_snapshot(snapshot).unwrap(),
            database,
            page,
            embed,
            view: view_id,
            title: property_id(10),
            status: property_id(11),
            amount: property_id(12),
            tags: property_id(13),
            due: property_id(14),
            done: property_id(15),
            link: property_id(16),
            notes: property_id(17),
            todo,
            doing,
            finished,
            red,
            blue,
            rows: 0,
        }
    }

    pub fn add_row(&mut self, title: &str) -> BlockId {
        self.add_row_with_id(BlockId::new(), title)
    }

    pub fn add_row_with_id(&mut self, id: BlockId, title: &str) -> BlockId {
        self.rows += 1;
        let mut row = block(
            id,
            BlockKind::Page { icon: None },
            Parent::Database(self.database),
            key(self.rows),
        );
        row.text = RichText::plain(title);
        self.workspace
            .children
            .entry(row.parent)
            .or_default()
            .insert((row.sort_key.clone(), row.id));
        self.workspace.blocks.insert(row.id, row);
        id
    }

    pub fn set(&mut self, row: BlockId, property: PropertyId, value: PropertyValue) {
        let operation = Operation::SetValue {
            row,
            property,
            value: Some(value),
        };
        self.workspace
            .apply(&Transaction::new(vec![operation]))
            .unwrap();
    }

    pub fn view_mut(&mut self) -> &mut View {
        self.workspace.views.get_mut(&self.view).unwrap()
    }
}
