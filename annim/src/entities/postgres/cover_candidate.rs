use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "cover_candidate")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub candidate_id: Uuid,
    pub release_db_id: i32,
    pub disc_number: i16,
    pub source_kind: String,
    pub source_release_revision_db_id: Option<i32>,
    pub submitted_url: Option<String>,
    pub canonical_url: Option<String>,
    pub effective_url: Option<String>,
    pub state: String,
    pub asset_db_id: Option<i32>,
    pub attempt_count: i32,
    pub lease_token: Option<Uuid>,
    pub lease_expires_at: Option<DateTime>,
    pub next_attempt_at: Option<DateTime>,
    pub last_http_status: Option<i32>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub fetched_at: Option<DateTime>,
    pub row_version: i64,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
