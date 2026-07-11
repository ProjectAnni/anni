use sea_orm_migration::prelude::*;

use super::{
    m20260712_000007_create_catalog_collection::CollectionCopy,
    m20260712_000008_create_catalog_sources::CatalogSourceRelease,
};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260712_000010_add_catalog_lookup_indexes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx-collection-copy-release")
                    .table(CollectionCopy::Table)
                    .col(CollectionCopy::ReleaseDbId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-catalog-source-release-linked-release")
                    .table(CatalogSourceRelease::Table)
                    .col(CatalogSourceRelease::LinkedReleaseDbId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-catalog-source-release-linked-release")
                    .table(CatalogSourceRelease::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx-collection-copy-release")
                    .table(CollectionCopy::Table)
                    .to_owned(),
            )
            .await
    }
}
