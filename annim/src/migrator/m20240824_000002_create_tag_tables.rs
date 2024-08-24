use sea_orm::{EnumIter, Iterable};
use sea_orm_migration::{prelude::*, schema::*};

use super::{
    helper::pk_foreign,
    m20240817_000001_create_basic_tables::{Album, Disc, Track},
};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240824_000002_create_tag_tables"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // TagInfo
        manager
            .create_table(
                Table::create()
                    .table(TagInfo::Table)
                    .col(pk_auto(TagInfo::Id))
                    .col(string(TagInfo::Name))
                    .col(enumeration(
                        TagInfo::Type,
                        Alias::new("type"),
                        TagType::iter(),
                    ))
                    .col(timestamp(TagInfo::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(TagInfo::UpdatedAt).default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-tag-name-type")
                    .table(TagInfo::Table)
                    .col(TagInfo::Name)
                    .col(TagInfo::Type)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // TagRelation
        manager
            .create_table(
                Table::create()
                    .table(TagRelation::Table)
                    .col(pk_auto(TagRelation::Id))
                    .col(pk_foreign(TagRelation::TagDbId))
                    .col(pk_foreign(TagRelation::ParentTagDbId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-tag-relation-tag")
                            .from(TagRelation::Table, TagRelation::TagDbId)
                            .to(TagInfo::Table, TagInfo::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-tag-relation-parent-tag")
                            .from(TagRelation::Table, TagRelation::ParentTagDbId)
                            .to(TagInfo::Table, TagInfo::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // AlbumTagRelation
        manager
            .create_table(
                Table::create()
                    .table(AlbumTagRelation::Table)
                    .col(pk_auto(AlbumTagRelation::Id))
                    .col(pk_foreign(AlbumTagRelation::TagDbId))
                    .col(pk_foreign(AlbumTagRelation::AlbumDbId))
                    .col(pk_foreign(AlbumTagRelation::DiscDbId))
                    .col(pk_foreign(AlbumTagRelation::TrackDbId))
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk-album-tag-relation-album")
                            .from(AlbumTagRelation::Table, AlbumTagRelation::AlbumDbId)
                            .to(Album::Table, Album::Id),
                    )
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk-album-tag-relation-disc")
                            .from(AlbumTagRelation::Table, AlbumTagRelation::DiscDbId)
                            .to(Disc::Table, Disc::Id),
                    )
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk-album-tag-relation-track")
                            .from(AlbumTagRelation::Table, AlbumTagRelation::TrackDbId)
                            .to(Track::Table, Track::Id),
                    )
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk-album-tag-relation-tag")
                            .from(AlbumTagRelation::Table, AlbumTagRelation::TagDbId)
                            .to(TagInfo::Table, TagInfo::Id),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-tag-relation-album")
                    .table(AlbumTagRelation::Table)
                    .col(AlbumTagRelation::AlbumDbId)
                    .col(AlbumTagRelation::TagDbId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-tag-relation-disc")
                    .table(AlbumTagRelation::Table)
                    .col(AlbumTagRelation::DiscDbId)
                    .col(AlbumTagRelation::TagDbId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-tag-relation-track")
                    .table(AlbumTagRelation::Table)
                    .col(AlbumTagRelation::TrackDbId)
                    .col(AlbumTagRelation::TagDbId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // TagInfo Table
        manager
            .drop_table(Table::drop().table(TagInfo::Table).to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-tag-name-type").to_owned())
            .await?;

        // TagRelation Table
        manager
            .drop_table(Table::drop().table(TagRelation::Table).to_owned())
            .await?;

        // AlbumTagRelation Table
        manager
            .drop_table(Table::drop().table(AlbumTagRelation::Table).to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-tag-relation-album").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-tag-relation-disc").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-tag-relation-track").to_owned())
            .await?;
        Ok(())
    }
}

/// Table to store basic tag information.
#[derive(Iden)]
pub enum TagInfo {
    Table,
    /// Tag Table ID
    Id,
    /// Tag Name
    Name,
    /// Tag Type
    Type,

    // Metadata
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden, EnumIter)]
enum TagType {
    Artist,
    Group,
    Animation,
    Radio,
    Series,
    Project,
    Game,
    Organization,
    Others,
}

/// Store the relationship between tags.
#[derive(Iden)]
pub enum TagRelation {
    Table,
    Id,
    /// Tag Table ID
    TagDbId,
    /// Parent Tag Table ID
    ParentTagDbId,
}

#[derive(Iden)]
pub enum AlbumTagRelation {
    Table,
    Id,
    /// Tag Table ID
    TagDbId,
    /// Album Table ID
    AlbumDbId,
    /// Disc Table ID
    DiscDbId,
    /// Track Table ID
    TrackDbId,
}
