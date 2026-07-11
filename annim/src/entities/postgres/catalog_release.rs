use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "catalog_release")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub release_id: Uuid,
    pub artist_db_id: i32,
    pub title: String,
    pub edition: Option<String>,
    pub catalog: Option<String>,
    pub release_date: Option<String>,
    pub kind: String,
    pub wanted: bool,
    pub unavailable: bool,
    pub matched_album_id: Option<Uuid>,
    pub active_ingest_job_id: Option<Uuid>,
    pub notes: Option<String>,
    pub row_version: i64,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
