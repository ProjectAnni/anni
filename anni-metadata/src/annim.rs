#[cynic::schema("annim")]
pub(crate) mod schema {}
cynic::impl_scalar!(uuid::Uuid, schema::UUID);
cynic::impl_scalar!(chrono::DateTime<chrono::Utc>, schema::DateTime);
cynic::impl_scalar!(serde_json::Value, schema::JSON);

pub(crate) type Uuid = uuid::Uuid;
pub(crate) type DateTime = chrono::DateTime<chrono::Utc>;
pub(crate) type Json = serde_json::Value;

pub use client::AnnimClient;
pub use schema::ID;

mod client;
// TODO: make this private
pub mod mutation;
pub mod query;
