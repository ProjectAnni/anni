use base64::{prelude::BASE64_STANDARD, write::EncoderStringWriter, Engine};
use sea_orm::sea_query::ValueTuple;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct Cursor(Vec<CursorValue>);

impl Cursor {
    pub(crate) fn new(values: Vec<sea_orm::Value>) -> Self {
        let values: Vec<_> = values.into_iter().map(Into::into).collect();
        Cursor(values)
    }

    pub(crate) fn from_str(input: &str) -> anyhow::Result<Self> {
        let bytes = BASE64_STANDARD.decode(input)?;
        let cursor: Cursor = rmp_serde::from_slice(&bytes)?;
        Ok(cursor)
    }

    pub(crate) fn to_string(&self) -> String {
        let mut writer = EncoderStringWriter::new(&BASE64_STANDARD);
        rmp_serde::encode::write(&mut writer, self).unwrap();
        writer.into_inner()
    }

    pub(crate) fn into_value_tuple(self) -> ValueTuple {
        ValueTuple::Many(self.into_inner())
    }

    fn into_inner(self) -> Vec<sea_orm::Value> {
        self.0.into_iter().map(Into::into).collect()
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) enum CursorValue {
    #[serde(rename = "t")]
    /// t -> tiny
    TinyInt(Option<i8>),
    /// m -> mini
    #[serde(rename = "m")]
    SmallInt(Option<i16>),
    /// n -> normal
    #[serde(rename = "n")]
    Int(Option<i32>),
    /// b -> bigint
    #[serde(rename = "b")]
    BigInt(Option<i64>),
    #[serde(rename = "T")]
    TinyUnsigned(Option<u8>),
    #[serde(rename = "M")]
    SmallUnsigned(Option<u16>),
    #[serde(rename = "N")]
    Unsigned(Option<u32>),
    #[serde(rename = "B")]
    BigUnsigned(Option<u64>),
    /// s -> string
    #[serde(rename = "s")]
    String(Option<Box<String>>),
}

impl From<sea_orm::Value> for CursorValue {
    fn from(value: sea_orm::Value) -> Self {
        match value {
            sea_orm::Value::TinyInt(value) => CursorValue::TinyInt(value),
            sea_orm::Value::SmallInt(value) => CursorValue::SmallInt(value),
            sea_orm::Value::Int(value) => CursorValue::Int(value),
            sea_orm::Value::BigInt(value) => CursorValue::BigInt(value),
            sea_orm::Value::TinyUnsigned(value) => CursorValue::TinyUnsigned(value),
            sea_orm::Value::SmallUnsigned(value) => CursorValue::SmallUnsigned(value),
            sea_orm::Value::Unsigned(value) => CursorValue::Unsigned(value),
            sea_orm::Value::BigUnsigned(value) => CursorValue::BigUnsigned(value),
            sea_orm::Value::String(value) => CursorValue::String(value),
            _ => panic!("Unsupported value type"),
        }
    }
}

impl From<CursorValue> for sea_orm::Value {
    fn from(value: CursorValue) -> Self {
        match value {
            CursorValue::TinyInt(value) => sea_orm::Value::TinyInt(value),
            CursorValue::SmallInt(value) => sea_orm::Value::SmallInt(value),
            CursorValue::Int(value) => sea_orm::Value::Int(value),
            CursorValue::BigInt(value) => sea_orm::Value::BigInt(value),
            CursorValue::TinyUnsigned(value) => sea_orm::Value::TinyUnsigned(value),
            CursorValue::SmallUnsigned(value) => sea_orm::Value::SmallUnsigned(value),
            CursorValue::Unsigned(value) => sea_orm::Value::Unsigned(value),
            CursorValue::BigUnsigned(value) => sea_orm::Value::BigUnsigned(value),
            CursorValue::String(value) => sea_orm::Value::String(value),
        }
    }
}
