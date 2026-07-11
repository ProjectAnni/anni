use anni_catalog::ReleaseKind;
use anni_ingest::Digest;
use sea_orm_migration::{prelude::*, schema::*};

use super::{
    helper::pk_foreign, m20240817_000001_create_basic_tables::Album,
    m20260712_000005_create_ingest_job::IngestJob,
};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260712_000007_create_catalog_collection"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CatalogArtist::Table)
                    .col(pk_auto(CatalogArtist::Id))
                    .col(uuid_uniq(CatalogArtist::ArtistId))
                    .col(string(CatalogArtist::DisplayName))
                    .col(string_null(CatalogArtist::SortName))
                    .col(text_null(CatalogArtist::Notes))
                    .col(big_integer(CatalogArtist::RowVersion).default(1))
                    .col(timestamp(CatalogArtist::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(CatalogArtist::UpdatedAt).default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CatalogRelease::Table)
                    .col(pk_auto(CatalogRelease::Id))
                    .col(uuid_uniq(CatalogRelease::ReleaseId))
                    .col(pk_foreign(CatalogRelease::ArtistDbId))
                    .col(string(CatalogRelease::Title))
                    .col(string_null(CatalogRelease::Edition))
                    .col(string_null(CatalogRelease::Catalog))
                    // A string preserves year-only and year-month precision.
                    .col(string_null(CatalogRelease::ReleaseDate))
                    .col(string(CatalogRelease::Kind).default(ReleaseKind::Album.as_str()))
                    .col(boolean(CatalogRelease::Wanted).default(false))
                    .col(boolean(CatalogRelease::Unavailable).default(false))
                    .col(uuid_null(CatalogRelease::MatchedAlbumId))
                    .col(uuid_null(CatalogRelease::ActiveIngestJobId))
                    .col(text_null(CatalogRelease::Notes))
                    .col(big_integer(CatalogRelease::RowVersion).default(1))
                    .col(timestamp(CatalogRelease::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(CatalogRelease::UpdatedAt).default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-catalog-release-artist")
                            .from(CatalogRelease::Table, CatalogRelease::ArtistDbId)
                            .to(CatalogArtist::Table, CatalogArtist::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-catalog-release-album")
                            .from(CatalogRelease::Table, CatalogRelease::MatchedAlbumId)
                            .to(Album::Table, Album::AlbumId)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-catalog-release-ingest-job")
                            .from(CatalogRelease::Table, CatalogRelease::ActiveIngestJobId)
                            .to(IngestJob::Table, IngestJob::JobId)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-catalog-release-artist")
                    .table(CatalogRelease::Table)
                    .col(CatalogRelease::ArtistDbId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CollectionCopy::Table)
                    .col(pk_auto(CollectionCopy::Id))
                    .col(uuid_uniq(CollectionCopy::CopyId))
                    .col(pk_foreign(CollectionCopy::ReleaseDbId))
                    .col(string(CollectionCopy::SourceKind))
                    .col(string(CollectionCopy::SourceLabel))
                    // Locators can contain private tracker or local path data;
                    // public GraphQL output must not expose this column.
                    .col(text_null(CollectionCopy::PrivateLocator))
                    .col(string(CollectionCopy::Codec))
                    .col(integer_null(CollectionCopy::SampleRateHz))
                    .col(small_integer_null(CollectionCopy::BitDepth))
                    .col(small_integer_null(CollectionCopy::Channels))
                    .col(integer_null(CollectionCopy::TrackCount))
                    .col(big_integer_null(CollectionCopy::ByteLength))
                    .col(binary_len_null(
                        CollectionCopy::ManifestDigest,
                        Digest::LENGTH as u32,
                    ))
                    .col(boolean(CollectionCopy::QualityVerified).default(false))
                    .col(uuid_null(CollectionCopy::IngestJobId))
                    .col(text_null(CollectionCopy::Notes))
                    .col(timestamp(CollectionCopy::AcquiredAt).default(Expr::current_timestamp()))
                    .col(timestamp(CollectionCopy::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(CollectionCopy::UpdatedAt).default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-collection-copy-release")
                            .from(CollectionCopy::Table, CollectionCopy::ReleaseDbId)
                            .to(CatalogRelease::Table, CatalogRelease::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-collection-copy-ingest-job")
                            .from(CollectionCopy::Table, CollectionCopy::IngestJobId)
                            .to(IngestJob::Table, IngestJob::JobId)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CollectionCopy::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CatalogRelease::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CatalogArtist::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub(crate) enum CatalogArtist {
    Table,
    Id,
    ArtistId,
    DisplayName,
    SortName,
    Notes,
    RowVersion,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub(crate) enum CatalogRelease {
    Table,
    Id,
    ReleaseId,
    ArtistDbId,
    Title,
    Edition,
    Catalog,
    ReleaseDate,
    Kind,
    Wanted,
    Unavailable,
    MatchedAlbumId,
    ActiveIngestJobId,
    Notes,
    RowVersion,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub(crate) enum CollectionCopy {
    Table,
    Id,
    CopyId,
    ReleaseDbId,
    SourceKind,
    SourceLabel,
    PrivateLocator,
    Codec,
    SampleRateHz,
    BitDepth,
    Channels,
    TrackCount,
    ByteLength,
    ManifestDigest,
    QualityVerified,
    IngestJobId,
    Notes,
    AcquiredAt,
    CreatedAt,
    UpdatedAt,
}
