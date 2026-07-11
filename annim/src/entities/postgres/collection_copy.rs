use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "collection_copy")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub copy_id: Uuid,
    pub release_db_id: i32,
    pub source_kind: String,
    pub source_label: String,
    pub private_locator: Option<String>,
    pub codec: String,
    pub sample_rate_hz: Option<i32>,
    pub bit_depth: Option<i16>,
    pub channels: Option<i16>,
    pub track_count: Option<i32>,
    pub byte_length: Option<i64>,
    pub manifest_digest: Option<Vec<u8>>,
    pub quality_verified: bool,
    pub ingest_job_id: Option<Uuid>,
    pub notes: Option<String>,
    pub acquired_at: DateTime,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
