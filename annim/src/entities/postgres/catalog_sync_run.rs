use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "catalog_sync_run")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub run_id: Uuid,
    pub source_db_id: i32,
    pub status: String,
    pub requested_cursor: Option<String>,
    pub result_cursor: Option<String>,
    pub observed_count: i32,
    pub error_message: Option<String>,
    pub coverage: String,
    pub started_from_root: bool,
    pub snapshot_complete: bool,
    pub lease_token: Option<Uuid>,
    pub lease_expires_at: Option<DateTime>,
    pub next_attempt_at: Option<DateTime>,
    pub attempt_count: i32,
    pub row_version: i64,
    pub created_at: DateTime,
    pub started_at: Option<DateTime>,
    pub finished_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
