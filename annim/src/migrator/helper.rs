use sea_orm::sea_query::{ColumnDef, IntoIden};
use sea_orm_migration::schema::{integer, integer_null};

pub fn pk_foreign<T: IntoIden>(name: T) -> ColumnDef {
    integer(name).take()
}

pub fn pk_foreign_null<T: IntoIden>(name: T) -> ColumnDef {
    integer_null(name).take()
}
