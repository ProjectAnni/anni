use sea_orm::sea_query::{ColumnDef, IntoIden};
use sea_orm_migration::schema::integer;

pub fn pk_foreign<T: IntoIden>(name: T) -> ColumnDef {
    integer(name).not_null().take()
}
