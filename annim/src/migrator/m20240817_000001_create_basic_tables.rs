use sea_orm::{EnumIter, Iterable};
use sea_orm_migration::{prelude::*, schema::*};

use super::helper::pk_foreign;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240817_000001_create_album_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create the `Album` table.
        manager
            .create_table(
                Table::create()
                    .table(Album::Table)
                    .col(pk_auto(Album::Id))
                    .col(uuid_uniq(Album::AlbumId))
                    .col(string(Album::Title))
                    .col(string_null(Album::Edition))
                    .col(string_null(Album::Catalog))
                    .col(string(Album::Artist))
                    // Release Date -> YYYY-MM-DD, e.g. 2024-08-17
                    //              -> YYYY-MM, e.g. 2024-08
                    //              -> YYYY, e.g. 2024
                    .col(integer(Album::ReleaseYear))
                    .col(small_integer_null(Album::ReleaseMonth))
                    .col(small_integer_null(Album::ReleaseDay))
                    .col(timestamp(Album::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(Album::UpdatedAt).default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        // Create the `Disc` table.
        manager
            .create_table(
                Table::create()
                    .table(Disc::Table)
                    .col(pk_auto(Disc::Id))
                    .col(pk_foreign(Disc::AlbumDbId))
                    .col(integer(Disc::Index))
                    .col(string_null(Disc::Title))
                    .col(string_null(Disc::Catalog))
                    .col(string_null(Disc::Artist))
                    .col(timestamp(Disc::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(Disc::UpdatedAt).default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk-disc_album")
                            .from(Disc::Table, Disc::AlbumDbId)
                            .to(Album::Table, Album::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Create the `Track` table.
        manager
            .create_table(
                Table::create()
                    .table(Track::Table)
                    .col(pk_auto(Track::Id))
                    .col(pk_foreign(Track::AlbumDbId))
                    .col(pk_foreign(Track::DiscDbId))
                    .col(integer(Track::Index))
                    .col(string(Track::Title))
                    .col(string(Track::Artist))
                    .col(json_null(Track::Artists))
                    .col(timestamp(Track::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(Track::UpdatedAt).default(Expr::current_timestamp()))
                    .col(enumeration(
                        Track::Type,
                        Alias::new("type"),
                        TrackType::iter(),
                    ))
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk-track_album")
                            .from(Track::Table, Track::AlbumDbId)
                            .to(Album::Table, Album::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-track_disc")
                            .from(Track::Table, Track::DiscDbId)
                            .to(Disc::Table, Disc::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Create the `AlbumId` Index.
        manager
            .create_index(
                Index::create()
                    .name("idx-album-id")
                    .table(Album::Table)
                    .col(Album::AlbumId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the `Album` table.
        manager
            .drop_table(Table::drop().table(Album::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Disc::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Track::Table).to_owned())
            .await?;
        // manager
        //     .drop_foreign_key(
        //         ForeignKey::create()
        //             .name("fk-disc_album")
        //             .from(Disc::Table, Disc::Album)
        //             .to(Album::Table, Album::Id),
        //     )
        //     .await?;

        // Drop the `AlbumId` Index.
        manager
            .drop_index(
                Index::drop()
                    .name("idx-album-id")
                    .table(Album::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
pub enum Album {
    Table,
    /// Album Table ID
    Id,
    /// Album ID (UUID)
    AlbumId,
    Title,
    Edition,
    Catalog,
    Artist,
    ReleaseYear,
    ReleaseMonth,
    ReleaseDay,

    // Metadata
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum Disc {
    Table,
    /// Disc Table ID
    Id,
    /// Album Table ID
    AlbumDbId,
    /// Disc Index, starting from 0
    Index,
    Title,
    Artist,
    Catalog,

    // Metadata
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum Track {
    Table,
    /// Track Table ID
    Id,
    /// Album Table ID
    AlbumDbId,
    /// Disc Table ID
    DiscDbId,
    /// Track Index, starting from 0
    Index,
    Title,
    Artist,
    /// Additonal artist(s) on the track
    Artists,
    Type,

    // Metadata
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden, EnumIter)]
enum TrackType {
    Normal,
    Instrumental,
    Absolute,
    Drama,
    Radio,
    Vocal,
    Unknown,
}
