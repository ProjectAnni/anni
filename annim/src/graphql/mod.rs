mod input;
mod types;

use std::str::FromStr;

use anyhow::Ok;
use async_graphql::{
    connection::{Connection, Edge},
    Context, EmptySubscription, Object, Schema,
};
use input::AlbumsBy;
use sea_orm::{
    prelude::Uuid, sea_query::NullOrdering, ActiveModelTrait, ActiveValue, ColumnTrait,
    DatabaseConnection, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use seaography::{apply_pagination, CursorInput, PaginationInput};
use types::{AlbumInfo, DiscInfo, MetadataOrganizeLevel, TagInfo, TagType, TrackInfo};

use crate::{
    auth::require_auth,
    entities::{album, disc, tag_info, track},
};

pub type MetadataSchema = Schema<MetadataQuery, MetadataMutation, EmptySubscription>;

pub fn build_schema(db: DatabaseConnection) -> MetadataSchema {
    Schema::build(MetadataQuery, MetadataMutation, EmptySubscription)
        .data(db)
        .finish()
}

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

    async fn albums<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        by: AlbumsBy,
        after: Option<String>,
        // before: Option<String>,
        first: Option<u64>,
        // last: Option<i32>,
    ) -> anyhow::Result<Connection<String, AlbumInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();

        let query = match by {
            AlbumsBy::AlbumIds(album_ids) => {
                album::Entity::find().filter(album::Column::AlbumId.is_in(album_ids))
            }
            AlbumsBy::RecentlyCreated(limit) => album::Entity::find()
                .order_by_desc(album::Column::CreatedAt)
                .limit(limit),
            AlbumsBy::RecentlyUpdated(limit) => album::Entity::find()
                .order_by_desc(album::Column::UpdatedAt)
                .limit(limit),
            AlbumsBy::RecentlyReleased(limit) => album::Entity::find()
                .order_by_desc(album::Column::ReleaseYear)
                .order_by_with_nulls(album::Column::ReleaseMonth, Order::Desc, NullOrdering::Last)
                .order_by_with_nulls(album::Column::ReleaseDay, Order::Desc, NullOrdering::Last)
                .limit(limit),
            AlbumsBy::Keyword(_) => unimplemented!(),
            AlbumsBy::OrganizeLevel(level) => {
                album::Entity::find().filter(album::Column::Level.eq(level.to_string()))
            }
        };

        let pagination = PaginationInput {
            cursor: Some(CursorInput {
                cursor: after,
                limit: first.unwrap_or(20),
            }),
            page: None,
            offset: None,
        };
        let conn = apply_pagination::<album::Entity>(db, query, pagination).await?;
        let mut connection = Connection::new(
            conn.page_info.has_previous_page,
            conn.page_info.has_next_page,
        );
        connection.edges.extend(
            conn.edges
                .into_iter()
                .map(|e| Edge::new(e.cursor, AlbumInfo(e.node))),
        );
        Ok(connection)
    }

    async fn tag<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        tag_name: String,
        tag_type: Option<TagType>,
    ) -> anyhow::Result<Vec<TagInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let model = tag_info::Entity::find()
            .filter(match tag_type {
                Some(tag_type) => tag_info::Column::Name
                    .eq(tag_name)
                    .and(tag_info::Column::Type.eq(tag_type.to_string())),
                None => tag_info::Column::Name.eq(tag_name),
            })
            .all(db)
            .await?;
        Ok(model.into_iter().map(|model| TagInfo(model)).collect())
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
        require_auth(ctx)?;
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
            extra: ActiveValue::set(input.extra),
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
        require_auth(ctx)?;
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
        require_auth(ctx)?;
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
        require_auth(ctx)?;
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
    ///
    /// This method only works if the organize level of the album is INITIAL.
    async fn replace_album_discs<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::ReplaceAlbumDiscsInput,
    ) -> anyhow::Result<Option<AlbumInfo>> {
        require_auth(ctx)?;
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(album) = album::Entity::find()
            .filter(album::Column::Id.eq(input.id.parse::<i32>()?))
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let level = MetadataOrganizeLevel::from_str(&album.level)?;
        if level != MetadataOrganizeLevel::Initial {
            anyhow::bail!("Cannot replace discs of an album with organize level {level:?}",);
        }

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
    ///
    /// This method only works if the organize level of the album is INITIAL.
    async fn replace_disc_tracks<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::ReplaceDiscTracksInput,
    ) -> anyhow::Result<Option<DiscInfo>> {
        require_auth(ctx)?;
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(disc) = disc::Entity::find()
            .filter(disc::Column::Id.eq(input.id.parse::<i32>()?))
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let album_db_id = disc.album_db_id;
        let level: String = album::Entity::find()
            .select_only()
            .column(album::Column::Level)
            .filter(album::Column::Id.eq(album_db_id))
            .into_tuple()
            .one(db)
            .await?
            .unwrap();
        let level = MetadataOrganizeLevel::from_str(&level)?;
        if level != MetadataOrganizeLevel::Initial {
            anyhow::bail!("Cannot replace discs of an album with organize level {level:?}",);
        }

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

    /// Update organize level of an album.
    ///
    /// The organize level should only increase. However, it is not enforced by the server.
    async fn update_organize_level<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::UpdateAlbumOrganizeLevelInput,
    ) -> anyhow::Result<Option<AlbumInfo>> {
        require_auth(ctx)?;
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(album) = album::Entity::find()
            .filter(album::Column::Id.eq(input.id.parse::<i32>()?))
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let mut album: album::ActiveModel = album.into();
        album.level = ActiveValue::set(input.level.to_string());
        album.updated_at = ActiveValue::set(chrono::Utc::now());

        let album = album.update(db).await?;
        Ok(Some(AlbumInfo(album)))
    }
}
