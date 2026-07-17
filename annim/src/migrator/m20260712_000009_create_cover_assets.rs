use anni_catalog::CoverCandidateState;
use anni_ingest::Digest;
use sea_orm_migration::{prelude::*, schema::*};

use super::{
    helper::pk_foreign, m20260712_000007_create_catalog_collection::CatalogRelease,
    m20260712_000008_create_catalog_sources::CatalogSourceReleaseRevision,
};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260712_000009_create_cover_assets"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CoverAsset::Table)
                    .col(pk_auto(CoverAsset::Id))
                    .col(uuid_uniq(CoverAsset::AssetId))
                    // The immutable original is addressed by its bytes, not
                    // by a mutable remote URL or filename.
                    .col(binary_len_uniq(
                        CoverAsset::ContentSha256,
                        Digest::LENGTH as u32,
                    ))
                    .col(string_uniq(CoverAsset::StorageKey))
                    .col(string(CoverAsset::MediaType))
                    .col(integer(CoverAsset::Width))
                    .col(integer(CoverAsset::Height))
                    .col(big_integer(CoverAsset::ByteLength))
                    .col(timestamp(CoverAsset::FetchedAt))
                    .col(timestamp(CoverAsset::VerifiedAt))
                    .col(timestamp(CoverAsset::CreatedAt).default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CoverCandidate::Table)
                    .col(pk_auto(CoverCandidate::Id))
                    .col(uuid_uniq(CoverCandidate::CandidateId))
                    .col(pk_foreign(CoverCandidate::ReleaseDbId))
                    // Zero means the release-level cover; positive values are
                    // one-based disc numbers.
                    .col(small_integer(CoverCandidate::DiscNumber).default(0))
                    .col(string(CoverCandidate::SourceKind))
                    .col(integer_null(CoverCandidate::SourceReleaseRevisionDbId))
                    // All URL forms remain available for audit and fallback.
                    // Manual uploads may start with no URL and an asset.
                    .col(text_null(CoverCandidate::SubmittedUrl))
                    .col(text_null(CoverCandidate::CanonicalUrl))
                    .col(text_null(CoverCandidate::EffectiveUrl))
                    .col(
                        string(CoverCandidate::State)
                            .default(CoverCandidateState::Discovered.as_str()),
                    )
                    .col(integer_null(CoverCandidate::AssetDbId))
                    .col(integer(CoverCandidate::AttemptCount).default(0))
                    .col(uuid_null(CoverCandidate::LeaseToken))
                    .col(timestamp_null(CoverCandidate::LeaseExpiresAt))
                    .col(timestamp_null(CoverCandidate::NextAttemptAt))
                    .col(integer_null(CoverCandidate::LastHttpStatus))
                    .col(string_null(CoverCandidate::LastErrorCode))
                    // Error messages are sanitized before persistence because
                    // source URLs may contain secret query parameters.
                    .col(text_null(CoverCandidate::LastErrorMessage))
                    .col(timestamp_null(CoverCandidate::FetchedAt))
                    .col(big_integer(CoverCandidate::RowVersion).default(1))
                    .col(timestamp(CoverCandidate::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(CoverCandidate::UpdatedAt).default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-cover-candidate-release")
                            .from(CoverCandidate::Table, CoverCandidate::ReleaseDbId)
                            .to(CatalogRelease::Table, CatalogRelease::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-cover-candidate-asset")
                            .from(CoverCandidate::Table, CoverCandidate::AssetDbId)
                            .to(CoverAsset::Table, CoverAsset::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-cover-candidate-source-revision")
                            .from(
                                CoverCandidate::Table,
                                CoverCandidate::SourceReleaseRevisionDbId,
                            )
                            .to(
                                CatalogSourceReleaseRevision::Table,
                                CatalogSourceReleaseRevision::Id,
                            )
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-cover-candidate-release")
                    .table(CoverCandidate::Table)
                    .col(CoverCandidate::ReleaseDbId)
                    .col(CoverCandidate::DiscNumber)
                    .col(CoverCandidate::State)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-cover-candidate-claim")
                    .table(CoverCandidate::Table)
                    .col(CoverCandidate::State)
                    .col(CoverCandidate::NextAttemptAt)
                    .col(CoverCandidate::LeaseExpiresAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CoverSelection::Table)
                    .col(pk_auto(CoverSelection::Id))
                    .col(uuid_uniq(CoverSelection::SelectionId))
                    .col(pk_foreign(CoverSelection::ReleaseDbId))
                    .col(small_integer(CoverSelection::DiscNumber).default(0))
                    .col(pk_foreign(CoverSelection::CandidateDbId))
                    // Freeze the selected bytes independently from mutable
                    // candidate processing fields.
                    .col(pk_foreign(CoverSelection::AssetDbId))
                    .col(big_integer(CoverSelection::RowVersion).default(1))
                    .col(timestamp(CoverSelection::SelectedAt).default(Expr::current_timestamp()))
                    .col(timestamp(CoverSelection::UpdatedAt).default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-cover-selection-release")
                            .from(CoverSelection::Table, CoverSelection::ReleaseDbId)
                            .to(CatalogRelease::Table, CatalogRelease::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-cover-selection-candidate")
                            .from(CoverSelection::Table, CoverSelection::CandidateDbId)
                            .to(CoverCandidate::Table, CoverCandidate::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-cover-selection-asset")
                            .from(CoverSelection::Table, CoverSelection::AssetDbId)
                            .to(CoverAsset::Table, CoverAsset::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-cover-selection-scope")
                    .table(CoverSelection::Table)
                    .col(CoverSelection::ReleaseDbId)
                    .col(CoverSelection::DiscNumber)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CoverSelection::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CoverCandidate::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CoverAsset::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub(crate) enum CoverAsset {
    Table,
    Id,
    AssetId,
    ContentSha256,
    StorageKey,
    MediaType,
    Width,
    Height,
    ByteLength,
    FetchedAt,
    VerifiedAt,
    CreatedAt,
}

#[derive(Iden)]
pub(crate) enum CoverCandidate {
    Table,
    Id,
    CandidateId,
    ReleaseDbId,
    DiscNumber,
    SourceKind,
    SourceReleaseRevisionDbId,
    SubmittedUrl,
    CanonicalUrl,
    EffectiveUrl,
    State,
    AssetDbId,
    AttemptCount,
    LeaseToken,
    LeaseExpiresAt,
    NextAttemptAt,
    LastHttpStatus,
    LastErrorCode,
    LastErrorMessage,
    FetchedAt,
    RowVersion,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub(crate) enum CoverSelection {
    Table,
    Id,
    SelectionId,
    ReleaseDbId,
    DiscNumber,
    CandidateDbId,
    AssetDbId,
    RowVersion,
    SelectedAt,
    UpdatedAt,
}
