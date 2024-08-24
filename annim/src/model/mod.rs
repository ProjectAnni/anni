mod input;

use std::str::FromStr;

use async_graphql::{Context, EmptySubscription, Enum, Object, Schema, ID};
use sea_orm::{
    prelude::Uuid, ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, TransactionTrait,
};

use crate::entities::{album, disc, track};

pub type AppSchema = Schema<MetadataQuery, MetadataMutation, EmptySubscription>;

pub fn build_schema(db: DatabaseConnection) -> AppSchema {
    Schema::build(MetadataQuery, MetadataMutation, EmptySubscription)
        .data(db)
        .finish()
}

struct AlbumInfo(album::Model);

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

struct DiscInfo(disc::Model);

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

struct TrackInfo(track::Model);

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

// struct TagInfo<'tag>(&'tag TagRef<'tag>);

// #[Object]
// impl<'tag> TagInfo<'tag> {
//     async fn name(&self) -> &str {
//         self.0.name()
//     }

//     #[graphql(name = "type")]
//     async fn tag_type(&self) -> &str {
//         self.0.tag_type().as_ref()
//     }

//     async fn includes<'ctx>(&'tag self, ctx: &Context<'ctx>) -> Vec<TagInfo<'tag>>
//     where
//         'ctx: 'tag,
//     {
//         let manager = ctx.data_unchecked::<OwnedRepositoryManager>();
//         manager
//             .child_tags(self.0)
//             .into_iter()
//             .map(TagInfo)
//             .collect()
//     }

//     #[graphql(flatten)]
//     async fn detail<'ctx>(&self, ctx: &Context<'ctx>) -> TagDetail<'tag>
//     where
//         'ctx: 'tag,
//     {
//         let manager = ctx.data_unchecked::<OwnedRepositoryManager>();
//         TagDetail(manager.tag(self.0).unwrap())
//     }
// }

// struct TagDetail<'tag>(&'tag Tag);

// #[Object]
// impl<'tag> TagDetail<'tag> {
//     async fn names(&self) -> &HashMap<String, String> {
//         self.0.names()
//     }

//     async fn included_by(&self) -> Vec<TagInfo> {
//         self.0
//             .parents()
//             .iter()
//             .map(|t| TagInfo(t.deref()))
//             .collect()
//     }
// }

pub struct MetadataQuery;

#[Object]
impl MetadataQuery {
    async fn album<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        album_id: Uuid,
    ) -> anyhow::Result<Option<AlbumInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let model = album::Entity::find()
            .filter(album::Column::AlbumId.eq(album_id))
            .one(db)
            .await?;
        Ok(model.map(|model| AlbumInfo(model)))
    }

    // async fn albums_by_tag<'ctx>(&self, ctx: &Context<'ctx>, tag: String) -> Vec<AlbumInfo<'ctx>> {
    //     let tag = TagRef::from_cow_str(tag);
    //     let manager = ctx.data_unchecked::<OwnedRepositoryManager>();
    //     manager
    //         .albums_tagged_by(&tag)
    //         .map(|ids| {
    //             ids.iter()
    //                 .map(|album_id| manager.album(album_id).unwrap())
    //                 .map(AlbumInfo)
    //                 .collect()
    //         })
    //         .unwrap_or_else(|| Vec::new())
    // }
}

pub struct MetadataMutation;

#[Object]
impl MetadataMutation {
    async fn create_album<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::CreateAlbumInput,
    ) -> anyhow::Result<AlbumInfo> {
        let db = ctx.data::<DatabaseConnection>().unwrap();

        let txn = db.begin().await?;
        let album = album::ActiveModel {
            album_id: ActiveValue::set(input.album_id.unwrap_or_else(|| Uuid::new_v4())),
            title: ActiveValue::set(input.title),
            edition: ActiveValue::set(input.edition),
            catalog: ActiveValue::set(input.catalog),
            artist: ActiveValue::set(input.artist),
            release_year: ActiveValue::set(input.release_year),
            release_month: ActiveValue::set(input.release_month),
            release_day: ActiveValue::set(input.release_day),
            ..Default::default()
        };
        let album = album.insert(&txn).await?;

        // insert discs
        let album_db_id = album.id;
        for (index, input) in input.discs.into_iter().enumerate() {
            input.insert(&txn, album_db_id, index as i32).await?;
        }

        txn.commit().await?;
        Ok(AlbumInfo(album))
    }

    /// Update basic album information.
    /// Use this method to update basic album information such as title, artist and others.
    ///
    /// If you need to update disc or track information, use [updateDiscInfo] or [updateTrackInfo].
    /// If you need to change the structure of the album, use [replaceAlbumDiscs] or [replaceDiscTracks].
    async fn update_album_info<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::UpdateAlbumInfoInput,
    ) -> anyhow::Result<Option<AlbumInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(model) = album::Entity::find()
            .filter(album::Column::Id.eq(input.id.parse::<i32>()?))
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let album: album::ActiveModel = model.into();
        let album = input.update(album, db).await?;

        Ok(Some(AlbumInfo(album)))
    }

    /// Update basic disc information.
    async fn update_disc_info<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::UpdateDiscInfoInput,
    ) -> anyhow::Result<Option<DiscInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(model) = disc::Entity::find()
            .filter(disc::Column::Id.eq(input.id.parse::<i32>()?))
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let disc: disc::ActiveModel = model.into();
        let disc = input.update(disc, db).await?;

        Ok(Some(DiscInfo(disc)))
    }

    /// Update basic track information.
    async fn update_track_info<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::UpdateTrackInfoInput,
    ) -> anyhow::Result<Option<TrackInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(model) = track::Entity::find()
            .filter(track::Column::Id.eq(input.id.parse::<i32>()?))
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let track: track::ActiveModel = model.into();
        let track = input.update(track, db).await?;

        Ok(Some(TrackInfo(track)))
    }

    /// Replace discs of an album.
    async fn replace_album_discs<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::ReplaceAlbumDiscsInput,
    ) -> anyhow::Result<Option<AlbumInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(album) = album::Entity::find()
            .filter(album::Column::Id.eq(input.id.parse::<i32>()?))
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let album_db_id = album.id;
        let txn = db.begin().await?;

        // 1. remove old discs
        disc::Entity::delete_many()
            .filter(disc::Column::AlbumDbId.eq(album_db_id))
            .exec(&txn)
            .await?;

        // 2. remove old tracks
        track::Entity::delete_many()
            .filter(track::Column::AlbumDbId.eq(album_db_id))
            .exec(&txn)
            .await?;

        // 3. insert new discs and tracks
        for (index, disc) in input.discs.into_iter().enumerate() {
            disc.insert(&txn, album_db_id, index as i32).await?;
        }

        txn.commit().await?;

        Ok(Some(AlbumInfo(album)))
    }

    /// Replace tracks of a disc.
    async fn replace_disc_tracks<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::ReplaceDiscTracksInput,
    ) -> anyhow::Result<Option<DiscInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(disc) = disc::Entity::find()
            .filter(disc::Column::Id.eq(input.id.parse::<i32>()?))
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let album_db_id = disc.album_db_id;
        let disc_db_id = disc.id;
        let txn = db.begin().await?;

        // 1. remove old tracks
        track::Entity::delete_many()
            .filter(track::Column::DiscDbId.eq(disc_db_id))
            .exec(&txn)
            .await?;

        // 2. insert new tracks
        for (index, track) in input.tracks.into_iter().enumerate() {
            track
                .insert(&txn, album_db_id, disc_db_id, index as i32)
                .await?;
        }

        txn.commit().await?;

        Ok(Some(DiscInfo(disc)))
    }
}
