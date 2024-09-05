use sea_orm_migration::prelude::*;
mod helper;

mod m20240817_000001_create_basic_tables;
mod m20240824_000002_create_tag_tables;
mod m20240905_000003_add_tag_type_category;
mod m20240905_000004_album_extra_jsonb;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240817_000001_create_basic_tables::Migration),
            Box::new(m20240824_000002_create_tag_tables::Migration),
            Box::new(m20240905_000003_add_tag_type_category::Migration),
            Box::new(m20240905_000004_album_extra_jsonb::Migration),
        ]
    }
}
