use sea_orm::sea_query::{ColumnDef, IntoIden};
use sea_orm_migration::schema::big_integer;

/// Create a primary key column with auto-increment feature.
pub fn annim_pk_auto<T: IntoIden>(name: T) -> ColumnDef {
    big_integer(name).auto_increment().primary_key().take()
}

pub fn annim_pk_foreign<T: IntoIden>(name: T) -> ColumnDef {
    big_integer(name).not_null().take()
}
