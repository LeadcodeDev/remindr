use remindr_core::domain::{BlockId, DatabaseId, Emoji, OptionId, PropertyId, Uuid, ViewId};
use remindr_core::ports::StoreError;
use serde::Serialize;
use serde::de::DeserializeOwned;

macro_rules! named {
    ($ty:ty { $($variant:path => $name:literal),+ $(,)? }) => {
        impl Named for $ty {
            fn name(&self) -> &'static str {
                match self {
                    $($variant => $name),+
                }
            }

            fn from_name(raw: &str) -> Option<Self> {
                match raw {
                    $($name => Some($variant),)+
                    _ => None,
                }
            }
        }
    };
}

macro_rules! stored_id {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl StoredId for $ty {
                fn to_text(&self) -> String {
                    self.as_uuid().hyphenated().to_string()
                }

                fn parse(raw: &str) -> Result<Self, Invalid> {
                    Ok(Self::from_uuid(Uuid::parse_str(raw)?))
                }
            }
        )+
    };
}

mod block;
mod database;
mod rich_text;
mod value;
mod view;

pub(crate) use block::{BlockRow, encode_kind, encode_parent};
pub(crate) use database::{DatabaseRow, PropertyRow, encode_property_kind};
pub(crate) use rich_text::encode_text;
pub(crate) use value::{ValueRow, encode_value};
pub(crate) use view::{ViewRow, encode_view};

#[cfg(test)]
pub(crate) const AWKWARD_FLOATS: [f64; 6] = [
    1.1186231841461947,
    123456789.12345679,
    f64::MAX,
    f64::MIN_POSITIVE,
    5e-324,
    -0.0,
];

pub(crate) trait Named: Sized {
    fn name(&self) -> &'static str;
    fn from_name(raw: &str) -> Option<Self>;
}

pub(crate) trait StoredId: Sized {
    fn to_text(&self) -> String;
    fn parse(raw: &str) -> Result<Self, Invalid>;
}

stored_id!(BlockId, DatabaseId, PropertyId, ViewId, OptionId);

#[derive(Debug)]
pub(crate) struct Invalid(String);

impl Invalid {
    pub(crate) fn new(what: impl Into<String>) -> Self {
        Self(what.into())
    }
}

impl<E: std::error::Error> From<E> for Invalid {
    fn from(error: E) -> Self {
        Self(error.to_string())
    }
}

pub(crate) fn corrupt<E: Into<Invalid>>(column: &'static str) -> impl FnOnce(E) -> StoreError {
    move |error| StoreError::Corrupt(format!("{column}: {}", error.into().0))
}

pub(crate) fn to_json<T: Serialize>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(|error| StoreError::Write(Box::new(error)))
}

pub(crate) fn decode<T: DeserializeOwned>(
    column: &'static str,
    raw: &str,
) -> Result<T, StoreError> {
    serde_json::from_str(raw).map_err(corrupt(column))
}

pub(crate) fn decode_required<T: DeserializeOwned>(
    column: &'static str,
    raw: Option<&str>,
) -> Result<T, StoreError> {
    let raw = raw.ok_or_else(|| corrupt(column)(Invalid::new("missing")))?;
    decode(column, raw)
}

pub(crate) fn named<T: Named>(raw: &str, what: &str) -> Result<T, Invalid> {
    T::from_name(raw).ok_or_else(|| Invalid::new(format!("unknown {what} `{raw}`")))
}

pub(crate) fn emoji(raw: &str) -> Result<Emoji, Invalid> {
    Ok(Emoji::try_from(raw)?)
}

pub(crate) fn finite<S: serde::Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    if value.is_finite() {
        serializer.serialize_f64(*value)
    } else {
        Err(serde::ser::Error::custom("non-finite number"))
    }
}

pub(crate) fn finite_option<S: serde::Serializer>(
    value: &Option<f64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => finite(value, serializer),
        None => serializer.serialize_none(),
    }
}
