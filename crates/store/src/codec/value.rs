use std::borrow::Cow;

use remindr_core::domain::{BlockId, CalendarDate, OptionId, PropertyId, PropertyValue};
use remindr_core::ports::StoreError;
use rusqlite::Row;
use serde::{Deserialize, Serialize};

use super::{Invalid, StoredId, corrupt, decode, to_json};

const COLUMN: &str = "property_value.value";

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum ValueData<'a> {
    Text(Cow<'a, str>),
    Number(#[serde(serialize_with = "super::finite")] f64),
    Select(String),
    MultiSelect(Vec<String>),
    Date(String),
    Checkbox(bool),
    Url(Cow<'a, str>),
}

pub(crate) struct ValueRow {
    row_id: String,
    property_id: String,
    value: String,
}

impl ValueRow {
    pub(crate) const SELECT: &str =
        "SELECT row_id, property_id, value FROM property_value ORDER BY row_id, property_id";

    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<ValueRow> {
        Ok(ValueRow {
            row_id: row.get(0)?,
            property_id: row.get(1)?,
            value: row.get(2)?,
        })
    }

    pub(crate) fn into_value(self) -> Result<(BlockId, PropertyId, PropertyValue), StoreError> {
        let row = BlockId::parse(&self.row_id).map_err(corrupt("property_value.row_id"))?;
        let property =
            PropertyId::parse(&self.property_id).map_err(corrupt("property_value.property_id"))?;
        Ok((row, property, decode_value(&self.value)?))
    }
}

pub(crate) fn encode_value(value: &PropertyValue) -> Result<String, StoreError> {
    let data = match value {
        PropertyValue::Text(text) => ValueData::Text(Cow::Borrowed(text.as_str())),
        PropertyValue::Number(number) => ValueData::Number(*number),
        PropertyValue::Select(option) => ValueData::Select(option.to_text()),
        PropertyValue::MultiSelect(options) => {
            ValueData::MultiSelect(options.iter().map(StoredId::to_text).collect())
        }
        PropertyValue::Date(date) => ValueData::Date(date_text(date)),
        PropertyValue::Checkbox(checked) => ValueData::Checkbox(*checked),
        PropertyValue::Url(url) => ValueData::Url(Cow::Borrowed(url.as_str())),
    };
    to_json(&data)
}

fn decode_value(raw: &str) -> Result<PropertyValue, StoreError> {
    let data: ValueData<'_> = decode(COLUMN, raw)?;
    value_of(data).map_err(corrupt(COLUMN))
}

fn value_of(data: ValueData<'_>) -> Result<PropertyValue, Invalid> {
    let value = match data {
        ValueData::Text(text) => PropertyValue::Text(text.into_owned()),
        ValueData::Number(number) => PropertyValue::Number(number),
        ValueData::Select(option) => PropertyValue::Select(OptionId::parse(&option)?),
        ValueData::MultiSelect(options) => PropertyValue::MultiSelect(
            options
                .iter()
                .map(|option| OptionId::parse(option))
                .collect::<Result<_, Invalid>>()?,
        ),
        ValueData::Date(date) => PropertyValue::Date(parse_date(&date)?),
        ValueData::Checkbox(checked) => PropertyValue::Checkbox(checked),
        ValueData::Url(url) => PropertyValue::Url(url.into_owned()),
    };
    Ok(value)
}

pub(super) fn date_text(date: &CalendarDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

pub(super) fn parse_date(raw: &str) -> Result<CalendarDate, Invalid> {
    let mut parts = raw.rsplitn(3, '-');
    let (Some(day), Some(month), Some(year)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(Invalid::new(format!("bad date `{raw}`")));
    };
    Ok(CalendarDate::new(
        year.parse()?,
        month.parse()?,
        day.parse()?,
    )?)
}

#[cfg(test)]
mod tests {
    use remindr_core::domain::{BlockId, CalendarDate, OptionId, PropertyId, PropertyValue};
    use remindr_core::ports::StoreError;

    use super::{ValueRow, date_text, decode_value, encode_value, parse_date};
    use crate::codec::{AWKWARD_FLOATS, StoredId};

    fn corrupt_message(error: StoreError) -> String {
        match error {
            StoreError::Corrupt(message) => message,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn every_value_kind_round_trips() {
        let values = [
            PropertyValue::Text("a \"b\" é".to_owned()),
            PropertyValue::Number(-12.5),
            PropertyValue::Number(0.0),
            PropertyValue::Select(OptionId::new()),
            PropertyValue::MultiSelect(vec![OptionId::new(), OptionId::new()]),
            PropertyValue::MultiSelect(vec![]),
            PropertyValue::Date(CalendarDate::new(2026, 10, 23).unwrap()),
            PropertyValue::Checkbox(true),
            PropertyValue::Url("https://x.dev".to_owned()),
        ];
        for value in values {
            assert_eq!(decode_value(&encode_value(&value).unwrap()).unwrap(), value);
        }
    }

    #[test]
    fn values_are_tagged_json() {
        assert_eq!(
            encode_value(&PropertyValue::Text("hi".to_owned())).unwrap(),
            r#"{"type":"text","value":"hi"}"#
        );
        assert_eq!(
            encode_value(&PropertyValue::Number(2.5)).unwrap(),
            r#"{"type":"number","value":2.5}"#
        );
        assert_eq!(
            encode_value(&PropertyValue::Date(CalendarDate::new(2026, 1, 5).unwrap())).unwrap(),
            r#"{"type":"date","value":"2026-01-05"}"#
        );
        assert_eq!(
            encode_value(&PropertyValue::Checkbox(false)).unwrap(),
            r#"{"type":"checkbox","value":false}"#
        );
        assert_eq!(
            encode_value(&PropertyValue::MultiSelect(vec![])).unwrap(),
            r#"{"type":"multi_select","value":[]}"#
        );
    }

    #[test]
    fn numbers_round_trip_bit_for_bit() {
        for number in AWKWARD_FLOATS
            .into_iter()
            .flat_map(|number| [number, -number])
        {
            let encoded = encode_value(&PropertyValue::Number(number)).unwrap();
            match decode_value(&encoded).unwrap() {
                PropertyValue::Number(decoded) => {
                    assert_eq!(decoded.to_bits(), number.to_bits(), "{number:e}: {encoded}");
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn non_finite_numbers_are_refused() {
        for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                encode_value(&PropertyValue::Number(number)),
                Err(StoreError::Write(_))
            ));
        }
    }

    #[test]
    fn dates_round_trip_through_text() {
        for (year, month, day) in [(2026, 10, 23), (2024, 2, 29), (5, 1, 1), (-44, 3, 15)] {
            let date = CalendarDate::new(year, month, day).unwrap();
            assert_eq!(parse_date(&date_text(&date)).unwrap(), date);
        }
        assert_eq!(
            date_text(&CalendarDate::new(2026, 1, 5).unwrap()),
            "2026-01-05"
        );
    }

    #[test]
    fn bad_dates_are_invalid() {
        for raw in [
            "",
            "2026",
            "2026-13-01",
            "2026-02-30",
            "x-y-z",
            "2026-1-1-1",
        ] {
            assert!(parse_date(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn unknown_type_and_bad_payload_are_corrupt() {
        for raw in [
            "nope",
            r#"{"type":"hologram","value":1}"#,
            r#"{"type":"number","value":"x"}"#,
            r#"{"type":"select","value":"not-a-uuid"}"#,
            r#"{"type":"date","value":"2026-02-30"}"#,
        ] {
            let message = corrupt_message(decode_value(raw).unwrap_err());
            assert!(message.starts_with("property_value.value: "), "{message}");
        }
    }

    #[test]
    fn row_decodes_with_its_ids() {
        let row_id = BlockId::new();
        let property = PropertyId::new();
        let row = ValueRow {
            row_id: row_id.to_text(),
            property_id: property.to_text(),
            value: r#"{"type":"checkbox","value":true}"#.to_owned(),
        };
        assert_eq!(
            row.into_value().unwrap(),
            (row_id, property, PropertyValue::Checkbox(true))
        );
    }

    #[test]
    fn row_with_bad_ids_is_corrupt() {
        let row = ValueRow {
            row_id: "nope".to_owned(),
            property_id: PropertyId::new().to_text(),
            value: r#"{"type":"checkbox","value":true}"#.to_owned(),
        };
        let message = corrupt_message(row.into_value().unwrap_err());
        assert!(message.starts_with("property_value.row_id: "), "{message}");
    }
}
