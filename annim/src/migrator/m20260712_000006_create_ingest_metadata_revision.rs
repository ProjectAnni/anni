use anni_ingest::Digest;
use sea_orm_migration::{prelude::*, schema::*};

use super::m20260712_000005_create_ingest_job::IngestJob;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260712_000006_create_ingest_metadata_revision"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IngestMetadataRevision::Table)
                    .col(pk_auto(IngestMetadataRevision::Id))
                    .col(uuid(IngestMetadataRevision::JobId))
                    .col(big_integer(IngestMetadataRevision::Revision))
                    // JSON is stored as text so SQLite and PostgreSQL use the
                    // same representation and preserve every Unicode scalar.
                    .col(text(IngestMetadataRevision::Document))
                    .col(binary_len(
                        IngestMetadataRevision::DocumentSha256,
                        Digest::LENGTH as u32,
                    ))
                    .col(
                        timestamp(IngestMetadataRevision::CreatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp(IngestMetadataRevision::UpdatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-ingest-metadata-revision-job")
                            .from(IngestMetadataRevision::Table, IngestMetadataRevision::JobId)
                            .to(IngestJob::Table, IngestJob::JobId)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-ingest-metadata-revision-identity")
                    .table(IngestMetadataRevision::Table)
                    .col(IngestMetadataRevision::JobId)
                    .col(IngestMetadataRevision::Revision)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(IngestMetadataRevision::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum IngestMetadataRevision {
    Table,
    Id,
    JobId,
    Revision,
    Document,
    DocumentSha256,
    CreatedAt,
    UpdatedAt,
}
