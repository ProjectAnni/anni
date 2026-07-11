use sea_orm_migration::prelude::*;
mod helper;

mod m20240817_000001_create_basic_tables;
mod m20240824_000002_create_tag_tables;
mod m20240905_000003_add_tag_type_category;
mod m20240905_000004_album_extra_jsonb;
mod m20260712_000005_create_ingest_job;
mod m20260712_000006_create_ingest_metadata_revision;
mod m20260712_000007_create_catalog_collection;
mod m20260712_000008_create_catalog_sources;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240817_000001_create_basic_tables::Migration),
            Box::new(m20240824_000002_create_tag_tables::Migration),
            Box::new(m20240905_000003_add_tag_type_category::Migration),
            Box::new(m20240905_000004_album_extra_jsonb::Migration),
            Box::new(m20260712_000005_create_ingest_job::Migration),
            Box::new(m20260712_000006_create_ingest_metadata_revision::Migration),
            Box::new(m20260712_000007_create_catalog_collection::Migration),
            Box::new(m20260712_000008_create_catalog_sources::Migration),
        ]
    }
}
