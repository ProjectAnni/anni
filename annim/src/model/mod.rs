mod input;
mod types;

use async_graphql::{Context, EmptySubscription, Object, Schema};
use sea_orm::{
    prelude::Uuid, ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter, TransactionTrait,
};
use types::{AlbumInfo, DiscInfo, TrackInfo};

use crate::entities::{album, disc, track};

pub type AppSchema = Schema<MetadataQuery, MetadataMutation, EmptySubscription>;

pub fn build_schema(db: DatabaseConnection) -> AppSchema {
    Schema::build(MetadataQuery, MetadataMutation, EmptySubscription)
        .data(db)
        .finish()
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
    /// Add the metatada of a full album to annim.
    async fn add_album<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::AddAlbumInput,
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
