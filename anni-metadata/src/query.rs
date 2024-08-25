use cynic::impl_scalar;

use crate::schema;

pub mod album;

pub(crate) type Uuid = uuid::Uuid;
pub(crate) type DateTime = chrono::DateTime<chrono::Utc>;
pub(crate) type Json = serde_json::Value;

impl_scalar!(uuid::Uuid, schema::UUID);
impl_scalar!(chrono::DateTime<chrono::Utc>, schema::DateTime);
impl_scalar!(serde_json::Value, schema::JSON);
