//! SeaORM mapping for persisted audio-ingestion workflow state.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "ingest_job")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub job_id: Uuid,
    pub state: String,
    pub metadata_revision: i64,
    pub approved_revision: Option<i64>,
    pub manifest_digest: Option<Vec<u8>>,
    pub plan_digest: Option<Vec<u8>>,
    pub verification_digest: Option<Vec<u8>>,
    pub row_version: i64,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
