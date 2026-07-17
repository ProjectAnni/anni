use anni_ingest::{Digest, JobState};
use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260712_000005_create_ingest_job"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IngestJob::Table)
                    .col(pk_auto(IngestJob::Id))
                    .col(uuid_uniq(IngestJob::JobId))
                    .col(string(IngestJob::State).default(JobState::Created.as_str()))
                    .col(big_integer(IngestJob::MetadataRevision).default(1))
                    .col(big_integer_null(IngestJob::ApprovedRevision))
                    .col(binary_len_null(
                        IngestJob::ManifestDigest,
                        Digest::LENGTH as u32,
                    ))
                    .col(binary_len_null(
                        IngestJob::PlanDigest,
                        Digest::LENGTH as u32,
                    ))
                    .col(binary_len_null(
                        IngestJob::VerificationDigest,
                        Digest::LENGTH as u32,
                    ))
                    // Used by the repository layer for optimistic concurrency.
                    .col(big_integer(IngestJob::RowVersion).default(1))
                    .col(timestamp(IngestJob::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(IngestJob::UpdatedAt).default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IngestJob::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub(crate) enum IngestJob {
    Table,
    Id,
    JobId,
    State,
    MetadataRevision,
    ApprovedRevision,
    ManifestDigest,
    PlanDigest,
    VerificationDigest,
    RowVersion,
    CreatedAt,
    UpdatedAt,
}
