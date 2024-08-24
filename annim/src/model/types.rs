use std::str::FromStr;

use async_graphql::{Context, Enum, Object, ID};
use sea_orm::{
    prelude::DateTimeUtc, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};

use crate::entities::{album, disc, track};

pub struct AlbumInfo(pub(crate) album::Model);

#[Object(name = "Album")]
impl AlbumInfo {
    async fn id(&self) -> ID {
        ID(self.0.id.to_string())
    }

    /// Unique UUID of the album.
    async fn album_id(&self) -> String {
        self.0.album_id.to_string()
    }

    /// Title of the album.
    async fn title(&self) -> &str {
        self.0.title.as_str()
    }

    /// Optional edition of the album.
    async fn edition(&self) -> Option<&str> {
        self.0.edition.as_deref()
    }

    /// Optional catalog number of the album.
    async fn catalog(&self) -> Option<&str> {
        self.0.catalog.as_deref()
    }

    /// Artist of the album.
    async fn artist(&self) -> &str {
        self.0.artist.as_str()
    }

    /// Release year of the album.
    #[graphql(name = "year")]
    async fn release_year(&self) -> i32 {
        self.0.release_year
    }

    /// Optional release month of the album.
    #[graphql(name = "month")]
    async fn release_month(&self) -> Option<i16> {
        self.0.release_month
    }

    /// Optional release day of the album.
    #[graphql(name = "day")]
    async fn release_day(&self) -> Option<i16> {
        self.0.release_day
    }

    async fn created_at(&self) -> &DateTimeUtc {
        &self.0.created_at
    }

    async fn updated_at(&self) -> &DateTimeUtc {
        &self.0.updated_at
    }

    /// Discs of the album.
    async fn discs<'ctx>(&self, ctx: &Context<'ctx>) -> anyhow::Result<Vec<DiscInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let models = disc::Entity::find()
            .filter(disc::Column::AlbumDbId.eq(self.0.id))
            .order_by_asc(disc::Column::Index)
            .all(db)
            .await?;
        Ok(models.into_iter().map(|model| DiscInfo(model)).collect())
    }
}

pub struct DiscInfo(pub(crate) disc::Model);

#[Object(name = "Disc")]
impl DiscInfo {
    async fn id(&self) -> ID {
        ID(self.0.id.to_string())
    }

    async fn index(&self) -> i32 {
        self.0.index
    }

    async fn title(&self) -> Option<&str> {
        self.0.title.as_deref()
    }

    async fn catalog(&self) -> Option<&str> {
        self.0.catalog.as_deref()
    }

    async fn artist(&self) -> Option<&str> {
        self.0.artist.as_deref()
    }

    async fn created_at(&self) -> &DateTimeUtc {
        &self.0.created_at
    }

    async fn updated_at(&self) -> &DateTimeUtc {
        &self.0.updated_at
    }

    async fn tracks<'ctx>(&self, ctx: &Context<'ctx>) -> anyhow::Result<Vec<TrackInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let models = track::Entity::find()
            .filter(
                track::Column::AlbumDbId
                    .eq(self.0.album_db_id)
                    .and(track::Column::DiscDbId.eq(self.0.id)),
            )
            .order_by_asc(track::Column::Index)
            .all(db)
            .await?;
        Ok(models.into_iter().map(|model| TrackInfo(model)).collect())
    }
}

pub struct TrackInfo(pub(crate) track::Model);

#[Object(name = "Track")]
impl TrackInfo {
    async fn id(&self) -> ID {
        ID(self.0.id.to_string())
    }

    async fn index(&self) -> i32 {
        self.0.index
    }

    async fn title(&self) -> &str {
        self.0.title.as_str()
    }

    async fn artist(&self) -> &str {
        self.0.artist.as_str()
    }

    async fn artists(&self) -> Option<&serde_json::Value> {
        self.0.artists.as_ref()
    }

    async fn created_at(&self) -> &DateTimeUtc {
        &self.0.created_at
    }

    async fn updated_at(&self) -> &DateTimeUtc {
        &self.0.updated_at
    }

    #[graphql(name = "type")]
    async fn track_type(&self) -> TrackType {
        TrackType::from_str(self.0.r#type.as_str()).unwrap()
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TrackType {
    Normal,
    Instrumental,
    Absolute,
    Drama,
    Radio,
    Vocal,
    Unknown,
}

impl ToString for TrackType {
    fn to_string(&self) -> String {
        match self {
            TrackType::Normal => "normal".to_string(),
            TrackType::Instrumental => "instrumental".to_string(),
            TrackType::Absolute => "absolute".to_string(),
            TrackType::Drama => "drama".to_string(),
            TrackType::Radio => "radio".to_string(),
            TrackType::Vocal => "vocal".to_string(),
            TrackType::Unknown => "unknown".to_string(),
        }
    }
}

impl FromStr for TrackType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "normal" => Ok(TrackType::Normal),
            "instrumental" => Ok(TrackType::Instrumental),
            "absolute" => Ok(TrackType::Absolute),
            "drama" => Ok(TrackType::Drama),
            "radio" => Ok(TrackType::Radio),
            "vocal" => Ok(TrackType::Vocal),
            _ => Ok(TrackType::Unknown),
        }
    }
}
