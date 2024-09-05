use extension::postgres::Type;
use sea_orm::DbErr;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240905_000003_add_tag_type_category"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            sea_orm::DatabaseBackend::Postgres => {
                manager
                    .alter_type(
                        Type::alter()
                            .name(Alias::new("tag_type"))
                            .add_value(Alias::new("category"))
                            .before(super::m20240824_000002_create_tag_tables::TagType::Others),
                    )
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Can not revert the type change
        Ok(())
    }
}
