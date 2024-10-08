//! `SeaORM` Entity, @generated by sea-orm-codegen 1.0.0

use super::sea_orm_active_enums::TagType;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "tag_info")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub r#type: TagType,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::album_tag_relation::Entity")]
    AlbumTagRelation,
}

impl Related<super::album_tag_relation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AlbumTagRelation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
