use anni_catalog::SyncRunStatus;
use anni_ingest::Digest;
use sea_orm_migration::{prelude::*, schema::*};

use super::{
    helper::pk_foreign,
    m20260712_000007_create_catalog_collection::{CatalogArtist, CatalogRelease},
};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260712_000008_create_catalog_sources"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CatalogSource::Table)
                    .col(pk_auto(CatalogSource::Id))
                    .col(uuid_uniq(CatalogSource::SourceId))
                    .col(pk_foreign(CatalogSource::ArtistDbId))
                    .col(string(CatalogSource::Kind))
                    .col(string(CatalogSource::Locator))
                    .col(string_null(CatalogSource::Storefront))
                    .col(string_null(CatalogSource::Locale))
                    .col(text_null(CatalogSource::ConfigurationDocument))
                    // Credentials live outside the database; this is only a
                    // reference understood by the worker runtime.
                    .col(string_null(CatalogSource::SecretRef))
                    .col(boolean(CatalogSource::Enabled).default(true))
                    .col(big_integer(CatalogSource::RowVersion).default(1))
                    .col(timestamp(CatalogSource::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(CatalogSource::UpdatedAt).default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-catalog-source-artist")
                            .from(CatalogSource::Table, CatalogSource::ArtistDbId)
                            .to(CatalogArtist::Table, CatalogArtist::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-catalog-source-identity")
                    .table(CatalogSource::Table)
                    .col(CatalogSource::ArtistDbId)
                    .col(CatalogSource::Kind)
                    .col(CatalogSource::Locator)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CatalogSyncRun::Table)
                    .col(pk_auto(CatalogSyncRun::Id))
                    .col(uuid_uniq(CatalogSyncRun::RunId))
                    .col(pk_foreign(CatalogSyncRun::SourceDbId))
                    .col(string(CatalogSyncRun::Status).default(SyncRunStatus::Queued.as_str()))
                    .col(text_null(CatalogSyncRun::RequestedCursor))
                    .col(text_null(CatalogSyncRun::ResultCursor))
                    .col(integer(CatalogSyncRun::ObservedCount).default(0))
                    .col(text_null(CatalogSyncRun::ErrorMessage))
                    .col(big_integer(CatalogSyncRun::RowVersion).default(1))
                    .col(timestamp(CatalogSyncRun::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp_null(CatalogSyncRun::StartedAt))
                    .col(timestamp_null(CatalogSyncRun::FinishedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-catalog-sync-run-source")
                            .from(CatalogSyncRun::Table, CatalogSyncRun::SourceDbId)
                            .to(CatalogSource::Table, CatalogSource::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CatalogSourceRelease::Table)
                    .col(pk_auto(CatalogSourceRelease::Id))
                    .col(uuid_uniq(CatalogSourceRelease::SourceReleaseId))
                    .col(pk_foreign(CatalogSourceRelease::SourceDbId))
                    .col(string(CatalogSourceRelease::ExternalReleaseId))
                    .col(text(CatalogSourceRelease::SourceUrl))
                    .col(integer_null(CatalogSourceRelease::LinkedReleaseDbId))
                    .col(
                        timestamp(CatalogSourceRelease::FirstSeenAt)
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp(CatalogSourceRelease::LastSeenAt)
                            .default(Expr::current_timestamp()),
                    )
                    .col(timestamp_null(CatalogSourceRelease::NotSeenSince))
                    .col(big_integer(CatalogSourceRelease::RowVersion).default(1))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-catalog-source-release-source")
                            .from(
                                CatalogSourceRelease::Table,
                                CatalogSourceRelease::SourceDbId,
                            )
                            .to(CatalogSource::Table, CatalogSource::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-catalog-source-release-target")
                            .from(
                                CatalogSourceRelease::Table,
                                CatalogSourceRelease::LinkedReleaseDbId,
                            )
                            .to(CatalogRelease::Table, CatalogRelease::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-catalog-source-release-external-id")
                    .table(CatalogSourceRelease::Table)
                    .col(CatalogSourceRelease::SourceDbId)
                    .col(CatalogSourceRelease::ExternalReleaseId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CatalogSourceReleaseRevision::Table)
                    .col(pk_auto(CatalogSourceReleaseRevision::Id))
                    .col(pk_foreign(CatalogSourceReleaseRevision::SourceReleaseDbId))
                    .col(big_integer(CatalogSourceReleaseRevision::Revision))
                    .col(uuid(CatalogSourceReleaseRevision::SyncRunId))
                    .col(text(CatalogSourceReleaseRevision::RawDocument))
                    .col(text(CatalogSourceReleaseRevision::ParsedDocument))
                    .col(binary_len(
                        CatalogSourceReleaseRevision::RawSha256,
                        Digest::LENGTH as u32,
                    ))
                    .col(
                        timestamp(CatalogSourceReleaseRevision::ObservedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-catalog-source-release-revision-release")
                            .from(
                                CatalogSourceReleaseRevision::Table,
                                CatalogSourceReleaseRevision::SourceReleaseDbId,
                            )
                            .to(CatalogSourceRelease::Table, CatalogSourceRelease::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-catalog-source-release-revision-run")
                            .from(
                                CatalogSourceReleaseRevision::Table,
                                CatalogSourceReleaseRevision::SyncRunId,
                            )
                            .to(CatalogSyncRun::Table, CatalogSyncRun::RunId)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-catalog-source-release-revision")
                    .table(CatalogSourceReleaseRevision::Table)
                    .col(CatalogSourceReleaseRevision::SourceReleaseDbId)
                    .col(CatalogSourceReleaseRevision::Revision)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CatalogSourceReleaseRevision::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(CatalogSourceRelease::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CatalogSyncRun::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CatalogSource::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub(crate) enum CatalogSource {
    Table,
    Id,
    SourceId,
    ArtistDbId,
    Kind,
    Locator,
    Storefront,
    Locale,
    ConfigurationDocument,
    SecretRef,
    Enabled,
    RowVersion,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub(crate) enum CatalogSyncRun {
    Table,
    Id,
    RunId,
    SourceDbId,
    Status,
    RequestedCursor,
    ResultCursor,
    ObservedCount,
    ErrorMessage,
    RowVersion,
    CreatedAt,
    StartedAt,
    FinishedAt,
}

#[derive(Iden)]
pub(crate) enum CatalogSourceRelease {
    Table,
    Id,
    SourceReleaseId,
    SourceDbId,
    ExternalReleaseId,
    SourceUrl,
    LinkedReleaseDbId,
    FirstSeenAt,
    LastSeenAt,
    NotSeenSince,
    RowVersion,
}

#[derive(Iden)]
pub(crate) enum CatalogSourceReleaseRevision {
    Table,
    Id,
    SourceReleaseDbId,
    Revision,
    SyncRunId,
    RawDocument,
    ParsedDocument,
    RawSha256,
    ObservedAt,
}
