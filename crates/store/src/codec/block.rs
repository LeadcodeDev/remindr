use std::borrow::Cow;

use remindr_core::domain::{
    Block, BlockId, BlockKind, CodeLanguage, DatabaseId, HeadingLevel, Parent, SortKey, Timestamp,
};
use remindr_core::ports::StoreError;
use rusqlite::Row;
use serde::{Deserialize, Serialize};

use super::rich_text::decode_text;
use super::{Invalid, StoredId, corrupt, decode_required, emoji, to_json};

const KIND: &str = "block.kind";
const KIND_DATA: &str = "block.kind_data";

#[derive(Serialize, Deserialize)]
struct CheckedData {
    checked: bool,
}

#[derive(Serialize, Deserialize)]
struct IconData<'a> {
    icon: Cow<'a, str>,
}

#[derive(Serialize, Deserialize)]
struct PageData<'a> {
    icon: Option<Cow<'a, str>>,
}

#[derive(Serialize, Deserialize)]
struct LanguageData<'a> {
    language: Cow<'a, str>,
}

#[derive(Serialize, Deserialize)]
struct TargetData {
    database: String,
}

pub(crate) struct BlockRow {
    id: String,
    kind: String,
    kind_data: Option<String>,
    parent_table: String,
    parent_id: Option<String>,
    sort_key: String,
    text: String,
    alive: bool,
    created_at: i64,
}

impl BlockRow {
    pub(crate) const SELECT: &str = "SELECT id, kind, kind_data, parent_table, parent_id, sort_key, text, alive, created_at FROM block ORDER BY id";

    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<BlockRow> {
        Ok(BlockRow {
            id: row.get(0)?,
            kind: row.get(1)?,
            kind_data: row.get(2)?,
            parent_table: row.get(3)?,
            parent_id: row.get(4)?,
            sort_key: row.get(5)?,
            text: row.get(6)?,
            alive: row.get(7)?,
            created_at: row.get(8)?,
        })
    }

    pub(crate) fn into_block(self) -> Result<Block, StoreError> {
        Ok(Block {
            id: BlockId::parse(&self.id).map_err(corrupt("block.id"))?,
            kind: decode_kind(&self.kind, self.kind_data.as_deref())?,
            parent: decode_parent(&self.parent_table, self.parent_id.as_deref())?,
            sort_key: SortKey::parse(&self.sort_key).map_err(corrupt("block.sort_key"))?,
            text: decode_text(&self.text)?,
            alive: self.alive,
            created_at: Timestamp::from_millis(self.created_at),
        })
    }
}

pub(crate) fn encode_kind(kind: &BlockKind) -> Result<(&'static str, Option<String>), StoreError> {
    let columns = match kind {
        BlockKind::Text => ("text", None),
        BlockKind::Heading(HeadingLevel::One) => ("header", None),
        BlockKind::Heading(HeadingLevel::Two) => ("sub_header", None),
        BlockKind::Heading(HeadingLevel::Three) => ("sub_sub_header", None),
        BlockKind::BulletedListItem => ("bulleted_list", None),
        BlockKind::NumberedListItem => ("numbered_list", None),
        BlockKind::ToDo { checked } => {
            ("to_do", Some(to_json(&CheckedData { checked: *checked })?))
        }
        BlockKind::Toggle => ("toggle", None),
        BlockKind::Quote => ("quote", None),
        BlockKind::Callout { icon } => (
            "callout",
            Some(to_json(&IconData {
                icon: Cow::Borrowed(icon.as_str()),
            })?),
        ),
        BlockKind::Code { language } => (
            "code",
            Some(to_json(&LanguageData {
                language: Cow::Borrowed(language.as_str()),
            })?),
        ),
        BlockKind::Divider => ("divider", None),
        BlockKind::Page { icon } => (
            "page",
            Some(to_json(&PageData {
                icon: icon.as_ref().map(|icon| Cow::Borrowed(icon.as_str())),
            })?),
        ),
        BlockKind::DatabaseView { database } => (
            "database_view",
            Some(to_json(&TargetData {
                database: database.to_text(),
            })?),
        ),
    };
    Ok(columns)
}

fn decode_kind(name: &str, data: Option<&str>) -> Result<BlockKind, StoreError> {
    if let Some(kind) = plain_kind(name) {
        return Ok(kind);
    }
    match name {
        "to_do" => {
            let CheckedData { checked } = decode_required(KIND_DATA, data)?;
            Ok(BlockKind::ToDo { checked })
        }
        "callout" => {
            let IconData { icon } = decode_required(KIND_DATA, data)?;
            let icon = emoji(&icon).map_err(corrupt(KIND_DATA))?;
            Ok(BlockKind::Callout { icon })
        }
        "code" => {
            let LanguageData { language } = decode_required(KIND_DATA, data)?;
            Ok(BlockKind::Code {
                language: CodeLanguage::new(&language),
            })
        }
        "page" => {
            let PageData { icon } = decode_required(KIND_DATA, data)?;
            let icon = icon
                .as_deref()
                .map(emoji)
                .transpose()
                .map_err(corrupt(KIND_DATA))?;
            Ok(BlockKind::Page { icon })
        }
        "database_view" => {
            let TargetData { database } = decode_required(KIND_DATA, data)?;
            let database = DatabaseId::parse(&database).map_err(corrupt(KIND_DATA))?;
            Ok(BlockKind::DatabaseView { database })
        }
        other => Err(corrupt(KIND)(Invalid::new(format!(
            "unknown kind `{other}`"
        )))),
    }
}

fn plain_kind(name: &str) -> Option<BlockKind> {
    let kind = match name {
        "text" => BlockKind::Text,
        "header" => BlockKind::Heading(HeadingLevel::One),
        "sub_header" => BlockKind::Heading(HeadingLevel::Two),
        "sub_sub_header" => BlockKind::Heading(HeadingLevel::Three),
        "bulleted_list" => BlockKind::BulletedListItem,
        "numbered_list" => BlockKind::NumberedListItem,
        "toggle" => BlockKind::Toggle,
        "quote" => BlockKind::Quote,
        "divider" => BlockKind::Divider,
        _ => return None,
    };
    Some(kind)
}

pub(crate) fn encode_parent(parent: &Parent) -> (&'static str, Option<String>) {
    match parent {
        Parent::Root => ("root", None),
        Parent::Block(id) => ("block", Some(id.to_text())),
        Parent::Database(id) => ("database", Some(id.to_text())),
    }
}

fn decode_parent(table: &str, id: Option<&str>) -> Result<Parent, StoreError> {
    match (table, id) {
        ("root", None) => Ok(Parent::Root),
        ("block", Some(id)) => BlockId::parse(id)
            .map(Parent::Block)
            .map_err(corrupt("block.parent_id")),
        ("database", Some(id)) => DatabaseId::parse(id)
            .map(Parent::Database)
            .map_err(corrupt("block.parent_id")),
        _ => Err(corrupt("block.parent_table")(Invalid::new(format!(
            "`{table}` does not match parent_id {id:?}"
        )))),
    }
}

#[cfg(test)]
mod tests {
    use remindr_core::domain::{
        BlockId, BlockKind, CodeLanguage, DatabaseId, Emoji, HeadingLevel, Parent, RichText,
        SortKey, Timestamp,
    };
    use remindr_core::ports::StoreError;

    use super::{BlockRow, decode_kind, decode_parent, encode_kind, encode_parent};
    use crate::codec::StoredId;

    fn emoji(raw: &str) -> Emoji {
        Emoji::try_from(raw).unwrap()
    }

    fn all_kinds() -> Vec<BlockKind> {
        vec![
            BlockKind::Text,
            BlockKind::Heading(HeadingLevel::One),
            BlockKind::Heading(HeadingLevel::Two),
            BlockKind::Heading(HeadingLevel::Three),
            BlockKind::BulletedListItem,
            BlockKind::NumberedListItem,
            BlockKind::ToDo { checked: true },
            BlockKind::ToDo { checked: false },
            BlockKind::Toggle,
            BlockKind::Quote,
            BlockKind::Callout {
                icon: emoji("💡")
            },
            BlockKind::Code {
                language: CodeLanguage::new("rust \"quoted\""),
            },
            BlockKind::Divider,
            BlockKind::Page { icon: None },
            BlockKind::Page {
                icon: Some(emoji("📄")),
            },
            BlockKind::DatabaseView {
                database: DatabaseId::new(),
            },
        ]
    }

    fn row(kind: &str, kind_data: Option<&str>) -> BlockRow {
        BlockRow {
            id: BlockId::new().to_text(),
            kind: kind.to_owned(),
            kind_data: kind_data.map(str::to_owned),
            parent_table: "root".to_owned(),
            parent_id: None,
            sort_key: "V".to_owned(),
            text: "[]".to_owned(),
            alive: true,
            created_at: 7,
        }
    }

    fn corrupt_message(error: StoreError) -> String {
        match error {
            StoreError::Corrupt(message) => message,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn every_kind_round_trips() {
        for kind in all_kinds() {
            let (name, data) = encode_kind(&kind).unwrap();
            assert_eq!(decode_kind(name, data.as_deref()).unwrap(), kind);
        }
    }

    #[test]
    fn kinds_use_notion_names_and_documented_data() {
        let encoded = |kind: BlockKind| encode_kind(&kind).unwrap();
        assert_eq!(
            encoded(BlockKind::Heading(HeadingLevel::One)),
            ("header", None)
        );
        assert_eq!(
            encoded(BlockKind::Heading(HeadingLevel::Two)),
            ("sub_header", None)
        );
        assert_eq!(
            encoded(BlockKind::Heading(HeadingLevel::Three)),
            ("sub_sub_header", None)
        );
        assert_eq!(
            encoded(BlockKind::ToDo { checked: true }),
            ("to_do", Some(r#"{"checked":true}"#.to_owned()))
        );
        assert_eq!(
            encoded(BlockKind::Callout {
                icon: emoji("💡")
            }),
            ("callout", Some(r#"{"icon":"💡"}"#.to_owned()))
        );
        assert_eq!(
            encoded(BlockKind::Page { icon: None }),
            ("page", Some(r#"{"icon":null}"#.to_owned()))
        );
        assert_eq!(
            encoded(BlockKind::Code {
                language: CodeLanguage::new("rust")
            }),
            ("code", Some(r#"{"language":"rust"}"#.to_owned()))
        );
    }

    #[test]
    fn unknown_block_kind_is_corrupt() {
        let message = corrupt_message(decode_kind("hologram", None).unwrap_err());
        assert!(message.starts_with("block.kind: "), "{message}");
    }

    #[test]
    fn missing_or_bad_kind_data_is_corrupt() {
        let missing = corrupt_message(decode_kind("to_do", None).unwrap_err());
        assert!(missing.starts_with("block.kind_data: "), "{missing}");
        let bad = corrupt_message(decode_kind("callout", Some(r#"{"icon":" "}"#)).unwrap_err());
        assert!(bad.starts_with("block.kind_data: "), "{bad}");
        let garbage = corrupt_message(decode_kind("code", Some("nope")).unwrap_err());
        assert!(garbage.starts_with("block.kind_data: "), "{garbage}");
    }

    #[test]
    fn parents_round_trip() {
        for parent in [
            Parent::Root,
            Parent::Block(BlockId::new()),
            Parent::Database(DatabaseId::new()),
        ] {
            let (table, id) = encode_parent(&parent);
            assert_eq!(decode_parent(table, id.as_deref()).unwrap(), parent);
        }
    }

    #[test]
    fn inconsistent_parent_is_corrupt() {
        let message = corrupt_message(decode_parent("block", None).unwrap_err());
        assert!(message.starts_with("block.parent_"), "{message}");
        let message = corrupt_message(decode_parent("block", Some("not-a-uuid")).unwrap_err());
        assert!(message.starts_with("block.parent_id: "), "{message}");
    }

    #[test]
    fn row_decodes_into_a_block() {
        let block = row("quote", None).into_block().unwrap();
        assert_eq!(block.kind, BlockKind::Quote);
        assert_eq!(block.parent, Parent::Root);
        assert_eq!(block.sort_key, SortKey::parse("V").unwrap());
        assert_eq!(block.text, RichText::default());
        assert!(block.alive);
        assert_eq!(block.created_at, Timestamp::from_millis(7));
    }

    #[test]
    fn row_with_bad_id_or_sort_key_is_corrupt() {
        let mut bad_id = row("quote", None);
        bad_id.id = "nope".to_owned();
        let message = corrupt_message(bad_id.into_block().unwrap_err());
        assert!(message.starts_with("block.id: "), "{message}");
        let mut bad_key = row("quote", None);
        bad_key.sort_key = "a0".to_owned();
        let message = corrupt_message(bad_key.into_block().unwrap_err());
        assert!(message.starts_with("block.sort_key: "), "{message}");
    }
}
