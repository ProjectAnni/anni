use async_graphql::{InputObject, InputType, OneofObject, ID};
use sea_orm::prelude::Uuid;

use crate::entities::album;

macro_rules! may_update_required {
    ($self: ident, $model: ident, $field: ident) => {
        if let Some(value) = $self.$field {
            $model.$field = sea_orm::ActiveValue::set(value);
        }
    };
}

macro_rules! may_update_optional {
    ($self: ident, $model: ident, $field: ident) => {
        if let Some(field) = $self.$field {
            $model.$field = sea_orm::ActiveValue::set(field.value);
        }
    };
}

#[derive(OneofObject)]
pub enum UpsertAlbumInfoInput {
    Insert(CreateAlbumInfoInput),
    Update(UpdateAlbumInfoInput),
}

#[derive(InputObject)]
pub struct CreateAlbumInfoInput {
    pub album_id: Option<Uuid>,
    pub title: String,
    pub edition: Option<String>,
    pub catalog: Option<String>,
    pub artist: String,
    pub release_year: i32,
    pub release_month: Option<i16>,
    pub release_day: Option<i16>,
}

#[derive(InputObject)]
pub struct UpdateAlbumInfoInput {
    // IDs can not be modified. They are used to identify the record.
    pub id: Option<ID>,
    pub album_id: Option<Uuid>,

    pub title: Option<String>,
    pub edition: Option<UpdateString>,
    pub catalog: Option<UpdateString>,
    pub artist: Option<String>,
    pub release_year: Option<i32>,
    pub release_month: Option<UpdateI16>,
    pub release_day: Option<UpdateI16>,
}

impl UpdateAlbumInfoInput {
    pub(crate) fn apply(self, mut model: album::ActiveModel) -> album::ActiveModel {
        may_update_required!(self, model, title);
        may_update_optional!(self, model, edition);
        may_update_optional!(self, model, catalog);
        may_update_required!(self, model, artist);
        may_update_required!(self, model, release_year);
        may_update_optional!(self, model, release_month);
        may_update_optional!(self, model, release_day);
        model
    }
}

pub type UpdateString = UpdateValue<String>;
pub type UpdateI16 = UpdateValue<i16>;

#[derive(InputObject)]
#[graphql(concrete(name = "UpdateString", params(String)))]
#[graphql(concrete(name = "UpdateI32", params(i32)))]
#[graphql(concrete(name = "UpdateI16", params(i16)))]
pub struct UpdateValue<T: InputType> {
    value: Option<T>,
}
