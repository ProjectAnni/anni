use anni_catalog::SyncCoverage;
use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260712_000011_harden_catalog_sync_runs"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite permits one ADD COLUMN per ALTER TABLE, so keep this
        // migration portable instead of emitting a backend-specific batch.
        for column in [
            string(CatalogSyncRun::Coverage)
                .default(SyncCoverage::DiscoveryOnly.as_str())
                .to_owned(),
            boolean(CatalogSyncRun::StartedFromRoot)
                .default(false)
                .to_owned(),
            boolean(CatalogSyncRun::SnapshotComplete)
                .default(false)
                .to_owned(),
            uuid_null(CatalogSyncRun::LeaseToken),
            timestamp_null(CatalogSyncRun::LeaseExpiresAt),
            timestamp_null(CatalogSyncRun::NextAttemptAt),
            integer(CatalogSyncRun::AttemptCount).default(0).to_owned(),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(CatalogSyncRun::Table)
                        .add_column(column)
                        .to_owned(),
                )
                .await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("idx-catalog-sync-run-claimable")
                    .table(CatalogSyncRun::Table)
                    .col(CatalogSyncRun::Status)
                    .col(CatalogSyncRun::NextAttemptAt)
                    .col(CatalogSyncRun::LeaseExpiresAt)
                    .col(CatalogSyncRun::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-catalog-sync-run-claimable")
                    .table(CatalogSyncRun::Table)
                    .to_owned(),
            )
            .await?;
        for column in [
            CatalogSyncRun::AttemptCount,
            CatalogSyncRun::NextAttemptAt,
            CatalogSyncRun::LeaseExpiresAt,
            CatalogSyncRun::LeaseToken,
            CatalogSyncRun::SnapshotComplete,
            CatalogSyncRun::StartedFromRoot,
            CatalogSyncRun::Coverage,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(CatalogSyncRun::Table)
                        .drop_column(column)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum CatalogSyncRun {
    Table,
    Status,
    Coverage,
    StartedFromRoot,
    SnapshotComplete,
    LeaseToken,
    LeaseExpiresAt,
    NextAttemptAt,
    AttemptCount,
    CreatedAt,
}
