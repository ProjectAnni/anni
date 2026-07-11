//! SeaORM mapping for versioned ingest metadata documents.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "ingest_metadata_revision")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub job_id: Uuid,
    pub revision: i64,
    pub document: String,
    pub document_digest: Vec<u8>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
