use sea_orm_migration::prelude::*;
mod helper;

mod m20240817_000001_create_basic_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20240817_000001_create_basic_tables::Migration)]
    }
}
