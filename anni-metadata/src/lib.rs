#[cynic::schema("annim")]
pub(crate) mod schema {}

pub(crate) type Uuid = uuid::Uuid;
pub(crate) type DateTime = chrono::DateTime<chrono::Utc>;
pub(crate) type Json = serde_json::Value;

cynic::impl_scalar!(uuid::Uuid, schema::UUID);
cynic::impl_scalar!(chrono::DateTime<chrono::Utc>, schema::DateTime);
cynic::impl_scalar!(serde_json::Value, schema::JSON);

mod client;
pub mod mutation;
pub mod query;

pub use client::AnnimClient;
