mod input;
mod types;

use std::str::FromStr;

use anyhow::Ok;
use async_graphql::{
    connection::{Connection, Edge},
    Context, EmptySubscription, Object, Schema, ID,
};
use input::{AlbumsBy, MetadataIDInput};
use sea_orm::{
    prelude::Uuid, sea_query::ValueTuple, ActiveModelTrait, ActiveValue, ColumnTrait,
    DatabaseConnection, EntityTrait, ModelTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use seaography::{decode_cursor, encode_cursor};
use types::{AlbumInfo, DiscInfo, MetadataOrganizeLevel, TagInfo, TagRelation, TagType, TrackInfo};

use crate::{
    auth::require_auth,
    entities::{album, album_tag_relation, disc, tag_info, tag_relation, track},
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

        let (mut query, columns) = match by {
            AlbumsBy::AlbumIds(album_ids) => (
                album::Entity::find()
                    .filter(album::Column::AlbumId.is_in(album_ids))
                    .cursor_by(album::Column::Id),
                vec![album::Column::Id],
            ),
            AlbumsBy::RecentlyCreated(limit) => (
                album::Entity::find()
                    .order_by_desc(album::Column::CreatedAt)
                    .limit(limit)
                    .cursor_by((album::Column::CreatedAt, album::Column::Id)),
                vec![album::Column::CreatedAt, album::Column::Id],
            ),
            AlbumsBy::RecentlyUpdated(limit) => (
                album::Entity::find()
                    .order_by_desc(album::Column::UpdatedAt)
                    .limit(limit)
                    .cursor_by((album::Column::UpdatedAt, album::Column::Id)),
                vec![album::Column::UpdatedAt, album::Column::Id],
            ),
            AlbumsBy::RecentlyReleased(limit) => {
                let mut cursor = album::Entity::find().limit(limit).cursor_by((
                    album::Column::ReleaseYear,
                    album::Column::ReleaseMonth,
                    album::Column::ReleaseDay,
                    album::Column::Id,
                ));
                cursor.desc();
                (
                    cursor,
                    vec![
                        album::Column::ReleaseYear,
                        album::Column::ReleaseMonth,
                        album::Column::ReleaseDay,
                        album::Column::Id,
                    ],
                )
            }
            AlbumsBy::Keyword(_) => unimplemented!(),
            AlbumsBy::OrganizeLevel(level) => (
                album::Entity::find()
                    .filter(album::Column::Level.eq(level.to_string()))
                    .cursor_by(album::Column::Id),
                vec![album::Column::Id],
            ),
            AlbumsBy::Tag(_) => unimplemented!(),
            // TODO: test this
            // AlbumsBy::Tag(tag_id) => (
            //     album_tag_relation::Entity::find_related()
            //         .filter(album_tag_relation::Column::TagDbId.eq(tag_id))
            //         .cursor_by(album::Column::Id),
            //     None,
            // ),
        };

        let limit = first.unwrap_or(20);

        if let Some(cursor) = after {
            let values = decode_cursor(&cursor)?;
            let cursor_values = ValueTuple::Many(values);
            query.after(cursor_values);
        }

        let mut data = query.first(limit + 1).all(db).await.unwrap();
        let mut has_next_page = false;

        if data.len() == limit as usize + 1 {
            data.pop();
            has_next_page = true;
        }

        let edges: Vec<_> = data
            .into_iter()
            .map(|model| {
                let values: Vec<sea_orm::Value> = columns
                    .iter()
                    .map(|variant| model.get(variant.clone()))
                    .collect();

                let cursor: String = encode_cursor(values);

                Edge::new(cursor, AlbumInfo(model))
            })
            .collect();

        let mut connection = Connection::new(false, has_next_page);
        connection.edges.extend(edges);
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
        let Some(model) = album::Entity::find_by_id(input.id.parse::<i32>()?)
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
        let Some(model) = disc::Entity::find_by_id(input.id.parse::<i32>()?)
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
        let Some(model) = track::Entity::find_by_id(input.id.parse::<i32>()?)
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
        let Some(album) = album::Entity::find_by_id(input.id.parse::<i32>()?)
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
        let Some(disc) = disc::Entity::find_by_id(input.id.parse::<i32>()?)
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let album_db_id = disc.album_db_id;
        let level: String = album::Entity::find_by_id(album_db_id)
            .select_only()
            .column(album::Column::Level)
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
        let Some(album) = album::Entity::find_by_id(input.id.parse::<i32>()?)
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let original_level = MetadataOrganizeLevel::from_str(&album.level)?;
        // Level change rules:
        match (original_level, input.level) {
            // 1 -> 2
            (MetadataOrganizeLevel::Initial, MetadataOrganizeLevel::Partial) => {}
            // 2 -> 3
            (MetadataOrganizeLevel::Partial, MetadataOrganizeLevel::Reviewed) => {}
            // 3 -> 4
            (MetadataOrganizeLevel::Reviewed, MetadataOrganizeLevel::Finished) => {}
            // 4 -> *
            (MetadataOrganizeLevel::Finished, _) => {}
            // 3 -> 1/2
            (
                MetadataOrganizeLevel::Reviewed,
                MetadataOrganizeLevel::Partial | MetadataOrganizeLevel::Initial,
            ) => {}
            // 2 -> 1
            (MetadataOrganizeLevel::Partial, MetadataOrganizeLevel::Initial) => {}
            // others are invalid
            _ => anyhow::bail!(
                "Cannot decrease organize level from {:?} to {:?}",
                original_level,
                input.level
            ),
        }

        // make Disc::Index and Track::Index sequential when organize level increases
        if original_level == MetadataOrganizeLevel::Initial {
            let txn = db.begin().await?;

            // 1. get all discs
            let discs = disc::Entity::find()
                .filter(disc::Column::AlbumDbId.eq(album.id))
                .order_by_asc(disc::Column::Index)
                .all(&txn)
                .await?;

            for (index, disc) in discs.into_iter().enumerate() {
                // 2. update disc indexes
                let disc_db_id = disc.id;
                let mut model: disc::ActiveModel = disc.into();
                model.index = ActiveValue::set(index as i32);
                model.update(&txn).await?;

                // 3. get all tracks
                let tracks = track::Entity::find()
                    .filter(track::Column::DiscDbId.eq(disc_db_id))
                    .order_by_asc(track::Column::Index)
                    .all(&txn)
                    .await?;

                // 4. update track indexes
                for (index, track) in tracks.into_iter().enumerate() {
                    let mut model: track::ActiveModel = track.into();
                    model.index = ActiveValue::set(index as i32);
                    model.update(&txn).await?;
                }
            }

            txn.commit().await?;
        }

        let mut album: album::ActiveModel = album.into();
        album.level = ActiveValue::set(input.level.to_string());
        album.updated_at = ActiveValue::set(chrono::Utc::now());

        let album = album.update(db).await?;
        Ok(Some(AlbumInfo(album)))
    }

    /// Add a new tag `type:name` to the database.
    async fn add_tag<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        name: String,
        r#type: TagType,
    ) -> anyhow::Result<TagInfo> {
        require_auth(ctx)?;
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let tag = tag_info::ActiveModel {
            name: ActiveValue::set(name),
            r#type: ActiveValue::set(r#type.to_string()),
            ..Default::default()
        };
        let tag = tag.insert(db).await?;
        Ok(TagInfo(tag))
    }

    async fn update_tag_relation(
        &self,
        ctx: &Context<'_>,
        tag_id: ID,
        parent_id: ID,
        remove: Option<bool>,
    ) -> anyhow::Result<Option<TagRelation>> {
        require_auth(ctx)?;
        let db = ctx.data::<DatabaseConnection>().unwrap();

        let remove = remove.unwrap_or(false);
        if remove {
            // remove relation
            tag_relation::Entity::delete_many()
                .filter(
                    tag_relation::Column::TagDbId
                        .eq(tag_id.parse::<i32>()?)
                        .and(tag_relation::Column::ParentTagDbId.eq(parent_id.parse::<i32>()?)),
                )
                .exec(db)
                .await?;
            Ok(None)
        } else {
            // create relation
            let relation = tag_relation::ActiveModel {
                tag_db_id: ActiveValue::set(tag_id.parse::<i32>()?),
                parent_tag_db_id: ActiveValue::set(parent_id.parse::<i32>()?),
                ..Default::default()
            };
            let relation = relation.insert(db).await?;
            Ok(Some(TagRelation(relation)))
        }
    }

    /// Update tags of an album, disc or track.
    async fn update_metadata_tags(
        &self,
        ctx: &Context<'_>,
        input: MetadataIDInput,
        tags: Vec<ID>,
    ) -> anyhow::Result<AlbumInfo> {
        require_auth(ctx)?;
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let tags_id = tags
            .iter()
            .map(|id| id.parse::<i32>())
            .collect::<Result<Vec<_>, _>>()?;

        let album_db_id = match input {
            MetadataIDInput::Album(album_db_id) => {
                let album_db_id = album_db_id.parse::<i32>()?;
                album_tag_relation::Entity::delete_many()
                    .filter(
                        album_tag_relation::Column::AlbumDbId
                            .eq(album_db_id)
                            .and(album_tag_relation::Column::DiscDbId.is_null())
                            .and(album_tag_relation::Column::TrackDbId.is_null()),
                    )
                    .exec(db)
                    .await?;
                album_tag_relation::Entity::insert_many(tags_id.into_iter().map(|tag_id| {
                    album_tag_relation::ActiveModel {
                        album_db_id: ActiveValue::set(album_db_id),
                        tag_db_id: ActiveValue::set(tag_id),
                        ..Default::default()
                    }
                }))
                .exec(db)
                .await?;

                album_db_id
            }
            MetadataIDInput::Disc(disc_db_id) => {
                let disc_db_id = disc_db_id.parse::<i32>()?;
                album_tag_relation::Entity::delete_many()
                    .filter(
                        album_tag_relation::Column::DiscDbId
                            .eq(disc_db_id)
                            .and(album_tag_relation::Column::TrackDbId.is_null()),
                    )
                    .exec(db)
                    .await?;

                let disc = disc::Entity::find_by_id(disc_db_id).one(db).await?.unwrap();
                album_tag_relation::Entity::insert_many(tags_id.into_iter().map(|tag_id| {
                    album_tag_relation::ActiveModel {
                        album_db_id: ActiveValue::set(disc.album_db_id),
                        disc_db_id: ActiveValue::set(disc_db_id),
                        tag_db_id: ActiveValue::set(tag_id),
                        ..Default::default()
                    }
                }))
                .exec(db)
                .await?;

                disc.album_db_id
            }
            MetadataIDInput::Track(track_db_id) => {
                let track_db_id = track_db_id.parse::<i32>()?;
                album_tag_relation::Entity::delete_many()
                    .filter(album_tag_relation::Column::TrackDbId.eq(track_db_id))
                    .exec(db)
                    .await?;

                let track = track::Entity::find_by_id(track_db_id)
                    .one(db)
                    .await?
                    .unwrap();
                album_tag_relation::Entity::insert_many(tags_id.into_iter().map(|tag_id| {
                    album_tag_relation::ActiveModel {
                        album_db_id: ActiveValue::set(track.album_db_id),
                        disc_db_id: ActiveValue::set(track.disc_db_id),
                        track_db_id: ActiveValue::set(track_db_id),
                        tag_db_id: ActiveValue::set(tag_id),
                        ..Default::default()
                    }
                }))
                .exec(db)
                .await?;

                track.album_db_id
            }
        };

        let album = album::Entity::find_by_id(album_db_id)
            .one(db)
            .await?
            .unwrap();
        Ok(AlbumInfo(album))
    }
}
