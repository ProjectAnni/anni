use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "catalog_source")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub source_id: Uuid,
    pub artist_db_id: i32,
    pub kind: String,
    pub locator: String,
    pub storefront: Option<String>,
    pub locale: Option<String>,
    pub configuration_document: Option<String>,
    pub secret_ref: Option<String>,
    pub enabled: bool,
    pub row_version: i64,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
