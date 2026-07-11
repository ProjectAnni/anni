use sea_orm::DbErr;
use sea_orm_migration::{prelude::*, schema::*};

use super::m20240817_000001_create_basic_tables::Album;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240905_000004_album_extra_jsonb"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite has a single JSON storage representation and does not support
        // ALTER COLUMN. The JSON -> JSONB distinction only exists on Postgres.
        if manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            manager
                .alter_table(
                    Table::alter()
                        .table(Album::Table)
                        .modify_column(json_binary_null(Album::Extra))
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            manager
                .alter_table(
                    Table::alter()
                        .table(Album::Table)
                        .modify_column(json_null(Album::Extra))
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}
