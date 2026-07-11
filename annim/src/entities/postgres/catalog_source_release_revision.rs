use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "catalog_source_release_revision")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub source_release_db_id: i32,
    pub revision: i64,
    pub sync_run_id: Uuid,
    pub raw_document: String,
    pub parsed_document: String,
    pub raw_sha256: Vec<u8>,
    pub observed_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
