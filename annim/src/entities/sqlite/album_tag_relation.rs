//! `SeaORM` Entity, @generated by sea-orm-codegen 1.0.0

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "album_tag_relation")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub tag_db_id: i32,
    pub album_db_id: i32,
    pub disc_db_id: Option<i32>,
    pub track_db_id: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::album::Entity",
        from = "Column::AlbumDbId",
        to = "super::album::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Album,
    #[sea_orm(
        belongs_to = "super::disc::Entity",
        from = "Column::DiscDbId",
        to = "super::disc::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Disc,
    #[sea_orm(
        belongs_to = "super::tag_info::Entity",
        from = "Column::TagDbId",
        to = "super::tag_info::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    TagInfo,
    #[sea_orm(
        belongs_to = "super::track::Entity",
        from = "Column::TrackDbId",
        to = "super::track::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Track,
}

impl Related<super::album::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Album.def()
    }
}

impl Related<super::disc::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Disc.def()
    }
}

impl Related<super::tag_info::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TagInfo.def()
    }
}

impl Related<super::track::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Track.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
