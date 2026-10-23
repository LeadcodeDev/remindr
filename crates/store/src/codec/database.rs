use std::borrow::Cow;
use std::collections::HashMap;

use remindr_core::domain::{
    Database, DatabaseId, NumberFormat, OptionColor, OptionId, Property, PropertyId, PropertyKind,
    Schema, SelectOption, SortKey, Timestamp,
};
use remindr_core::ports::StoreError;
use rusqlite::Row;
use serde::{Deserialize, Serialize};

use super::{Invalid, Named, StoredId, corrupt, decode_required, emoji, named, to_json};

const KIND: &str = "property.kind";
const KIND_DATA: &str = "property.kind_data";

named!(NumberFormat {
    NumberFormat::Plain => "plain",
    NumberFormat::Percent => "percent",
    NumberFormat::Euro => "euro",
    NumberFormat::Dollar => "dollar",
});

named!(OptionColor {
    OptionColor::Default => "default",
    OptionColor::Gray => "gray",
    OptionColor::Brown => "brown",
    OptionColor::Orange => "orange",
    OptionColor::Yellow => "yellow",
    OptionColor::Green => "green",
    OptionColor::Blue => "blue",
    OptionColor::Purple => "purple",
    OptionColor::Pink => "pink",
    OptionColor::Red => "red",
});

#[derive(Serialize, Deserialize)]
struct NumberData<'a> {
    format: Cow<'a, str>,
}

#[derive(Serialize, Deserialize)]
struct SelectData<'a> {
    options: Vec<OptionData<'a>>,
}

#[derive(Serialize, Deserialize)]
struct OptionData<'a> {
    id: String,
    name: Cow<'a, str>,
    color: Cow<'a, str>,
}

pub(crate) struct DatabaseRow {
    id: String,
    name: String,
    icon: Option<String>,
    sort_key: String,
    alive: bool,
    created_at: i64,
}

impl DatabaseRow {
    pub(crate) const SELECT: &str =
        "SELECT id, name, icon, sort_key, alive, created_at FROM database ORDER BY id";

    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<DatabaseRow> {
        Ok(DatabaseRow {
            id: row.get(0)?,
            name: row.get(1)?,
            icon: row.get(2)?,
            sort_key: row.get(3)?,
            alive: row.get(4)?,
            created_at: row.get(5)?,
        })
    }

    pub(crate) fn into_database(
        self,
        properties: &mut HashMap<DatabaseId, Vec<Property>>,
    ) -> Result<Database, StoreError> {
        let id = DatabaseId::parse(&self.id).map_err(corrupt("database.id"))?;
        Ok(Database {
            id,
            name: self.name,
            icon: self
                .icon
                .as_deref()
                .map(emoji)
                .transpose()
                .map_err(corrupt("database.icon"))?,
            schema: Schema {
                properties: properties.remove(&id).unwrap_or_default(),
            },
            sort_key: SortKey::parse(&self.sort_key).map_err(corrupt("database.sort_key"))?,
            alive: self.alive,
            created_at: Timestamp::from_millis(self.created_at),
        })
    }
}

pub(crate) struct PropertyRow {
    id: String,
    database_id: String,
    name: String,
    kind: String,
    kind_data: Option<String>,
}

impl PropertyRow {
    pub(crate) const SELECT: &str = "SELECT id, database_id, name, kind, kind_data FROM property ORDER BY database_id, position";

    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<PropertyRow> {
        Ok(PropertyRow {
            id: row.get(0)?,
            database_id: row.get(1)?,
            name: row.get(2)?,
            kind: row.get(3)?,
            kind_data: row.get(4)?,
        })
    }

    pub(crate) fn into_property(self) -> Result<(DatabaseId, Property), StoreError> {
        let database =
            DatabaseId::parse(&self.database_id).map_err(corrupt("property.database_id"))?;
        let property = Property {
            id: PropertyId::parse(&self.id).map_err(corrupt("property.id"))?,
            name: self.name,
            kind: decode_property_kind(&self.kind, self.kind_data.as_deref())?,
        };
        Ok((database, property))
    }
}

pub(crate) fn encode_property_kind(
    kind: &PropertyKind,
) -> Result<(&'static str, Option<String>), StoreError> {
    let columns = match kind {
        PropertyKind::Title => ("title", None),
        PropertyKind::Text => ("text", None),
        PropertyKind::Number { format } => (
            "number",
            Some(to_json(&NumberData {
                format: Cow::Borrowed(format.name()),
            })?),
        ),
        PropertyKind::Select { options } => ("select", Some(encode_options(options)?)),
        PropertyKind::MultiSelect { options } => ("multi_select", Some(encode_options(options)?)),
        PropertyKind::Date => ("date", None),
        PropertyKind::Checkbox => ("checkbox", None),
        PropertyKind::Url => ("url", None),
    };
    Ok(columns)
}

fn encode_options(options: &[SelectOption]) -> Result<String, StoreError> {
    let options = options
        .iter()
        .map(|option| OptionData {
            id: option.id.to_text(),
            name: Cow::Borrowed(option.name.as_str()),
            color: Cow::Borrowed(option.color.name()),
        })
        .collect();
    to_json(&SelectData { options })
}

fn decode_property_kind(name: &str, data: Option<&str>) -> Result<PropertyKind, StoreError> {
    match name {
        "title" => Ok(PropertyKind::Title),
        "text" => Ok(PropertyKind::Text),
        "number" => {
            let NumberData { format } = decode_required(KIND_DATA, data)?;
            let format = named(&format, "number format").map_err(corrupt(KIND_DATA))?;
            Ok(PropertyKind::Number { format })
        }
        "select" => Ok(PropertyKind::Select {
            options: decode_options(data)?,
        }),
        "multi_select" => Ok(PropertyKind::MultiSelect {
            options: decode_options(data)?,
        }),
        "date" => Ok(PropertyKind::Date),
        "checkbox" => Ok(PropertyKind::Checkbox),
        "url" => Ok(PropertyKind::Url),
        other => Err(corrupt(KIND)(Invalid::new(format!(
            "unknown kind `{other}`"
        )))),
    }
}

fn decode_options(data: Option<&str>) -> Result<Vec<SelectOption>, StoreError> {
    let SelectData { options } = decode_required(KIND_DATA, data)?;
    options
        .into_iter()
        .map(select_option)
        .collect::<Result<Vec<_>, Invalid>>()
        .map_err(corrupt(KIND_DATA))
}

fn select_option(data: OptionData<'_>) -> Result<SelectOption, Invalid> {
    Ok(SelectOption {
        id: OptionId::parse(&data.id)?,
        name: data.name.into_owned(),
        color: named(&data.color, "option color")?,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use remindr_core::domain::{
        DatabaseId, Emoji, NumberFormat, OptionColor, OptionId, Property, PropertyId, PropertyKind,
        SelectOption, SortKey, Timestamp,
    };
    use remindr_core::ports::StoreError;

    use super::{DatabaseRow, PropertyRow, decode_property_kind, encode_property_kind};
    use crate::codec::StoredId;

    const COLORS: [OptionColor; 10] = [
        OptionColor::Default,
        OptionColor::Gray,
        OptionColor::Brown,
        OptionColor::Orange,
        OptionColor::Yellow,
        OptionColor::Green,
        OptionColor::Blue,
        OptionColor::Purple,
        OptionColor::Pink,
        OptionColor::Red,
    ];

    fn options() -> Vec<SelectOption> {
        COLORS
            .iter()
            .map(|color| SelectOption {
                id: OptionId::new(),
                name: format!("opt \"{color:?}\""),
                color: *color,
            })
            .collect()
    }

    fn corrupt_message(error: StoreError) -> String {
        match error {
            StoreError::Corrupt(message) => message,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn every_property_kind_round_trips() {
        let kinds = [
            PropertyKind::Title,
            PropertyKind::Text,
            PropertyKind::Number {
                format: NumberFormat::Plain,
            },
            PropertyKind::Number {
                format: NumberFormat::Percent,
            },
            PropertyKind::Number {
                format: NumberFormat::Euro,
            },
            PropertyKind::Number {
                format: NumberFormat::Dollar,
            },
            PropertyKind::Select { options: options() },
            PropertyKind::MultiSelect { options: options() },
            PropertyKind::Select { options: vec![] },
            PropertyKind::Date,
            PropertyKind::Checkbox,
            PropertyKind::Url,
        ];
        for kind in kinds {
            let (name, data) = encode_property_kind(&kind).unwrap();
            assert_eq!(decode_property_kind(name, data.as_deref()).unwrap(), kind);
        }
    }

    #[test]
    fn kinds_use_documented_names_and_data() {
        assert_eq!(
            encode_property_kind(&PropertyKind::MultiSelect { options: vec![] }).unwrap(),
            ("multi_select", Some(r#"{"options":[]}"#.to_owned()))
        );
        assert_eq!(
            encode_property_kind(&PropertyKind::Number {
                format: NumberFormat::Euro
            })
            .unwrap(),
            ("number", Some(r#"{"format":"euro"}"#.to_owned()))
        );
        assert_eq!(
            encode_property_kind(&PropertyKind::Title).unwrap(),
            ("title", None)
        );
        let id = OptionId::new();
        let select = PropertyKind::Select {
            options: vec![SelectOption {
                id,
                name: "Todo".to_owned(),
                color: OptionColor::Gray,
            }],
        };
        assert_eq!(
            encode_property_kind(&select).unwrap().1,
            Some(format!(
                r#"{{"options":[{{"id":"{}","name":"Todo","color":"gray"}}]}}"#,
                id.to_text()
            ))
        );
    }

    #[test]
    fn unknown_kind_color_and_format_are_corrupt() {
        let kind = corrupt_message(decode_property_kind("hologram", None).unwrap_err());
        assert!(kind.starts_with("property.kind: "), "{kind}");
        let color = corrupt_message(
            decode_property_kind(
                "select",
                Some(&format!(
                    r#"{{"options":[{{"id":"{}","name":"x","color":"mauve"}}]}}"#,
                    OptionId::new().to_text()
                )),
            )
            .unwrap_err(),
        );
        assert!(color.starts_with("property.kind_data: "), "{color}");
        let format = corrupt_message(
            decode_property_kind("number", Some(r#"{"format":"yen"}"#)).unwrap_err(),
        );
        assert!(format.starts_with("property.kind_data: "), "{format}");
        let missing = corrupt_message(decode_property_kind("select", None).unwrap_err());
        assert!(missing.starts_with("property.kind_data: "), "{missing}");
    }

    #[test]
    fn property_row_decodes_with_its_database() {
        let database = DatabaseId::new();
        let property = PropertyId::new();
        let row = PropertyRow {
            id: property.to_text(),
            database_id: database.to_text(),
            name: "Done".to_owned(),
            kind: "checkbox".to_owned(),
            kind_data: None,
        };
        let (owner, decoded) = row.into_property().unwrap();
        assert_eq!(owner, database);
        assert_eq!(decoded.id, property);
        assert_eq!(decoded.kind, PropertyKind::Checkbox);
    }

    #[test]
    fn database_row_takes_its_properties() {
        let id = DatabaseId::new();
        let other = DatabaseId::new();
        let property = Property {
            id: PropertyId::new(),
            name: "Name".to_owned(),
            kind: PropertyKind::Title,
        };
        let mut properties = HashMap::from([(id, vec![property.clone()]), (other, vec![])]);
        let row = DatabaseRow {
            id: id.to_text(),
            name: "Tasks".to_owned(),
            icon: Some("✅".to_owned()),
            sort_key: "V".to_owned(),
            alive: false,
            created_at: 9,
        };
        let database = row.into_database(&mut properties).unwrap();
        assert_eq!(database.schema.properties, vec![property]);
        assert_eq!(database.icon, Some(Emoji::try_from("✅").unwrap()));
        assert_eq!(database.sort_key, SortKey::parse("V").unwrap());
        assert!(!database.alive);
        assert_eq!(database.created_at, Timestamp::from_millis(9));
        assert_eq!(properties.len(), 1);
    }

    #[test]
    fn database_row_with_bad_icon_is_corrupt() {
        let row = DatabaseRow {
            id: DatabaseId::new().to_text(),
            name: "x".to_owned(),
            icon: Some("two words".to_owned()),
            sort_key: "V".to_owned(),
            alive: true,
            created_at: 0,
        };
        let message = corrupt_message(row.into_database(&mut HashMap::new()).unwrap_err());
        assert!(message.starts_with("database.icon: "), "{message}");
    }
}
