use async_graphql::{InputObject, InputType, OneofObject, ID};
use sea_orm::{
    prelude::Uuid, ActiveModelTrait, ActiveValue, ConnectionTrait, DatabaseConnection, DbErr,
};

use super::types::{MetadataOrganizeLevel, TrackType};
use crate::{
    entities::{album, disc, helper::now, track},
    search::SearchWriter,
};

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

#[derive(InputObject)]
pub struct AddAlbumInput {
    pub album_id: Option<Uuid>,
    pub title: String,
    pub edition: Option<String>,
    pub catalog: Option<String>,
    pub artist: String,
    #[graphql(name = "year")]
    pub release_year: i32,
    #[graphql(name = "month")]
    pub release_month: Option<i16>,
    #[graphql(name = "day")]
    pub release_day: Option<i16>,
    pub extra: Option<serde_json::Value>,
    pub discs: Vec<CreateAlbumDiscInput>,
}

#[derive(InputObject)]
pub struct CreateAlbumDiscInput {
    pub title: Option<String>,
    pub catalog: Option<String>,
    pub artist: Option<String>,
    pub tracks: Vec<CreateAlbumTrackInput>,
}

impl CreateAlbumDiscInput {
    pub(crate) async fn insert<C: ConnectionTrait>(
        self,
        txn: &C,
        index_writer: &SearchWriter<'_>,
        album_db_id: i32,
        index: i32,
    ) -> anyhow::Result<disc::Model> {
        let disc = disc::ActiveModel {
            album_db_id: ActiveValue::set(album_db_id),
            index: ActiveValue::set(index),
            title: ActiveValue::set(self.title),
            catalog: ActiveValue::set(self.catalog),
            artist: ActiveValue::set(self.artist),
            ..Default::default()
        };
        let disc = disc.insert(txn).await?;

        index_writer.add_disc_info(&disc)?;

        let disc_db_id = disc.id;
        for (i, track) in self.tracks.into_iter().enumerate() {
            track
                .insert(txn, index_writer, album_db_id, disc_db_id, i as i32)
                .await?;
        }
        Ok(disc)
    }
}

#[derive(InputObject)]
pub struct CreateAlbumTrackInput {
    pub title: String,
    pub artist: String,
    pub r#type: TrackType,
}

impl CreateAlbumTrackInput {
    pub(crate) async fn insert<C: ConnectionTrait>(
        self,
        txn: &C,
        index_writer: &SearchWriter<'_>,
        album_db_id: i32,
        disc_db_id: i32,
        index: i32,
    ) -> anyhow::Result<track::Model> {
        let track = track::ActiveModel {
            album_db_id: ActiveValue::set(album_db_id),
            disc_db_id: ActiveValue::set(disc_db_id),
            index: ActiveValue::set(index),
            title: ActiveValue::set(self.title),
            artist: ActiveValue::set(self.artist),
            r#type: ActiveValue::set(self.r#type.into()),
            ..Default::default()
        };
        let track = track.insert(txn).await?;

        index_writer.add_track_info(&track)?;
        Ok(track)
    }
}

#[derive(InputObject)]
pub struct UpdateAlbumInfoInput {
    pub id: ID,

    pub title: Option<String>,
    pub edition: Option<UpdateString>,
    pub catalog: Option<UpdateString>,
    pub artist: Option<String>,
    #[graphql(name = "year")]
    pub release_year: Option<i32>,
    #[graphql(name = "month")]
    pub release_month: Option<UpdateI16>,
    #[graphql(name = "day")]
    pub release_day: Option<UpdateI16>,
    pub extra: Option<UpdateJson>,
}

impl UpdateAlbumInfoInput {
    pub(crate) async fn update(
        self,
        mut model: album::ActiveModel,
        db: &DatabaseConnection,
    ) -> Result<album::Model, DbErr> {
        may_update_required!(self, model, title);
        may_update_optional!(self, model, edition);
        may_update_optional!(self, model, catalog);
        may_update_required!(self, model, artist);
        may_update_required!(self, model, release_year);
        may_update_optional!(self, model, release_month);
        may_update_optional!(self, model, release_day);
        may_update_optional!(self, model, extra);

        model.updated_at = ActiveValue::set(now());
        model.update(db).await
    }
}

#[derive(InputObject)]
pub struct UpdateDiscInfoInput {
    pub id: ID,

    pub title: Option<UpdateString>,
    pub catalog: Option<UpdateString>,
    pub artist: Option<UpdateString>,
}

impl UpdateDiscInfoInput {
    pub(crate) async fn update(
        self,
        mut model: disc::ActiveModel,
        db: &DatabaseConnection,
    ) -> Result<disc::Model, DbErr> {
        may_update_optional!(self, model, title);
        may_update_optional!(self, model, catalog);
        may_update_optional!(self, model, artist);

        model.updated_at = ActiveValue::set(now());
        model.update(db).await
    }
}

#[derive(InputObject)]
pub struct UpdateTrackInfoInput {
    pub id: ID,

    pub title: Option<String>,
    pub artist: Option<String>,
    pub r#type: Option<TrackType>,
}

impl UpdateTrackInfoInput {
    pub(crate) async fn update(
        self,
        mut model: track::ActiveModel,
        db: &DatabaseConnection,
    ) -> Result<track::Model, DbErr> {
        may_update_required!(self, model, title);
        may_update_required!(self, model, artist);
        if let Some(r#type) = self.r#type {
            model.r#type = sea_orm::ActiveValue::set(r#type.into());
        }

        model.updated_at = ActiveValue::set(now());
        model.update(db).await
    }
}

#[derive(InputObject)]
pub struct ReplaceAlbumDiscsInput {
    pub id: ID,
    pub discs: Vec<CreateAlbumDiscInput>,
}

#[derive(InputObject)]
pub struct ReplaceDiscTracksInput {
    pub id: ID,
    pub tracks: Vec<CreateAlbumTrackInput>,
}

#[derive(InputObject)]
pub struct UpdateAlbumOrganizeLevelInput {
    pub id: ID,
    pub level: MetadataOrganizeLevel,
}

/// List albums by conditions.
#[derive(OneofObject)]
pub enum AlbumsBy {
    /// Get albums by their AlbumIDs.
    AlbumIds(Vec<Uuid>),
    /// Get albums recently added to the database.
    RecentlyCreated(u64 /* LIMIT */),
    /// Get albums recently updated in the database.
    RecentlyUpdated(u64 /* LIMIT */),
    /// Get albums recently released.
    RecentlyReleased(u64 /* LIMIT */),
    /// Search albums by keyword.
    Keyword(String),
    /// Get albums of certain organize level.
    OrganizeLevel(MetadataOrganizeLevel),
    /// Get albums of certain tag
    Tag(ID),
}

pub type UpdateString = UpdateValue<String>;
pub type UpdateI16 = UpdateValue<i16>;
pub type UpdateJson = UpdateValue<serde_json::Value>;

#[derive(InputObject)]
#[graphql(concrete(name = "UpdateString", params(String)))]
#[graphql(concrete(name = "UpdateI32", params(i32)))]
#[graphql(concrete(name = "UpdateI16", params(i16)))]
#[graphql(concrete(name = "UpdateJson", params(serde_json::Value)))]
pub struct UpdateValue<T: InputType> {
    value: Option<T>,
}

#[derive(OneofObject)]
pub enum MetadataIDInput {
    Album(ID),
    Disc(ID),
    Track(ID),
}
