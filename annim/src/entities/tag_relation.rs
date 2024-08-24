//! `SeaORM` Entity, @generated by sea-orm-codegen 1.0.0

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "tag_relation")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub tag_db_id: i32,
    pub parent_tag_db_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tag_info::Entity",
        from = "Column::ParentTagDbId",
        to = "super::tag_info::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    TagInfo2,
    #[sea_orm(
        belongs_to = "super::tag_info::Entity",
        from = "Column::TagDbId",
        to = "super::tag_info::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    TagInfo1,
}

impl ActiveModelBehavior for ActiveModel {}