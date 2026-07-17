use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "cover_asset")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub asset_id: Uuid,
    #[sea_orm(unique)]
    pub content_sha256: Vec<u8>,
    #[sea_orm(unique)]
    pub storage_key: String,
    pub media_type: String,
    pub width: i32,
    pub height: i32,
    pub byte_length: i64,
    pub fetched_at: DateTime,
    pub verified_at: DateTime,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
