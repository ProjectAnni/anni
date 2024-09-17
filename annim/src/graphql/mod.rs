mod cursor;
mod input;
pub mod types;

use std::{i64, str::FromStr};

use anyhow::Ok;
use async_graphql::{
    connection::{Connection, Edge},
    Context, EmptySubscription, Object, Schema, ID,
};
use cursor::Cursor;
use input::{AlbumsBy, MetadataIDInput};
use sea_orm::{
    prelude::Uuid, sea_query::IntoIden, ActiveModelTrait, ActiveValue, ColumnTrait,
    DatabaseConnection, EntityTrait, Identity, ModelTrait, QueryFilter, QueryOrder, QuerySelect,
    Related, TransactionTrait,
};
use tantivy::{
    collector::TopDocs,
    query::{BooleanQuery, Occur, PhraseQuery, QueryClone, TermQuery},
    Term,
};
use types::{
    AlbumInfo, DiscInfo, MetadataOrganizeLevel, TagInfo, TagRelation, TagType, TrackInfo,
    TrackSearchResult,
};

use crate::{
    auth::AdminGuard,
    entities::{album, album_tag_relation, disc, helper::now, tag_info, tag_relation, track},
    search::RepositorySearchManager,
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

        let (query, columns, desc) = match by {
            AlbumsBy::AlbumIds(album_ids) => (
                album::Entity::find().filter(album::Column::AlbumId.is_in(album_ids)),
                vec![album::Column::Id],
                false,
            ),
            AlbumsBy::RecentlyCreated(limit) => (
                album::Entity::find()
                    .order_by_desc(album::Column::CreatedAt)
                    .limit(limit),
                vec![album::Column::CreatedAt, album::Column::Id],
                false,
            ),
            AlbumsBy::RecentlyUpdated(limit) => (
                album::Entity::find()
                    .order_by_desc(album::Column::UpdatedAt)
                    .limit(limit),
                vec![album::Column::UpdatedAt, album::Column::Id],
                false,
            ),
            AlbumsBy::RecentlyReleased(limit) => (
                album::Entity::find().limit(limit),
                vec![
                    album::Column::ReleaseYear,
                    album::Column::ReleaseMonth,
                    album::Column::ReleaseDay,
                    album::Column::Id,
                ],
                true,
            ),
            AlbumsBy::Keyword(_) => unimplemented!(),
            AlbumsBy::OrganizeLevel(level) => (
                album::Entity::find().filter(album::Column::Level.eq(level.to_string())),
                vec![album::Column::Id],
                false,
            ),
            AlbumsBy::Tag(tag_id) => (
                album_tag_relation::Entity::find_related()
                    .filter(album_tag_relation::Column::TagDbId.eq(tag_id.parse::<i32>()?))
                    // dedup
                    .group_by(album_tag_relation::Column::AlbumDbId),
                vec![album::Column::Id],
                false,
            ),
        };

        let limit = first.unwrap_or(20);
        let mut query = query.cursor_by(Identity::Many(
            columns.iter().map(|r| r.into_iden()).collect(),
        ));
        if desc {
            query.desc();
        }

        if let Some(cursor) = after {
            let cursor = Cursor::from_str(&cursor)?;
            query.after(cursor.into_value_tuple());
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

                let cursor = Cursor::new(values);
                Edge::new(cursor.to_string(), AlbumInfo(model))
            })
            .collect();

        let mut connection = Connection::new(false, has_next_page);
        connection.edges.extend(edges);
        Ok(connection)
    }

    async fn tracks<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        keyword: String,
        #[graphql(default = 20)] count: usize,
        #[graphql(default = 0)] offset: usize,
    ) -> anyhow::Result<Vec<TrackSearchResult>> {
        let search_manager = ctx.data::<RepositorySearchManager>().unwrap();

        let query = search_manager.query_parser().parse_query(&keyword)?;
        let query_not = TermQuery::new(
            Term::from_field_i64(search_manager.fields.track_db_id, i64::MAX),
            Default::default(),
        );
        let query = BooleanQuery::new(vec![
            (Occur::Must, query),
            // AND -track_db_id:9223372036854775807
            (Occur::MustNot, Box::new(query_not)),
        ]);

        let searcher = search_manager.searcher();
        let top_docs = searcher.search(&query, &TopDocs::with_limit(count).and_offset(offset))?;

        let result: Vec<_> = top_docs
            .into_iter()
            .filter_map(|(score, addr)| {
                let doc = searcher.doc(addr).ok()?;
                let (album_db_id, disc_db_id, track_db_id) =
                    search_manager.deserialize_document(doc);
                if disc_db_id.is_none() || track_db_id.is_none() {
                    return None;
                }

                Some(TrackSearchResult {
                    score,
                    album_db_id,
                    disc_db_id: disc_db_id.unwrap(),
                    track_db_id: track_db_id.unwrap(),
                })
            })
            .collect();

        Ok(result)
    }

    // TODO: this query seems useless
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
}

pub struct MetadataMutation;

#[Object(guard = "AdminGuard")]
impl MetadataMutation {
    /// Add the metatada of a full album to annim.
    async fn add_album<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::AddAlbumInput,
        #[graphql(default = true)] commit: bool,
    ) -> anyhow::Result<AlbumInfo> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let searcher = ctx.data::<RepositorySearchManager>().unwrap();
        let index_writer = searcher.writer().await;

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

        index_writer.add_album_info(&album)?;

        // insert discs
        let album_db_id = album.id;
        for (index, input) in input.discs.into_iter().enumerate() {
            input
                .insert(&txn, &index_writer, album_db_id, index as i32)
                .await?;
        }

        txn.commit().await?;
        if commit {
            index_writer.commit().await?;
        }

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
        let Some(model) = album::Entity::find_by_id(input.id.parse::<i32>()?)
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let need_update_search_index = input.title.is_some() || input.artist.is_some();
        let album: album::ActiveModel = model.into();
        let album = input.update(album, db).await?;

        if need_update_search_index {
            let searcher = ctx.data::<RepositorySearchManager>().unwrap();
            let index_writer = searcher.writer().await;
            let query = PhraseQuery::new(vec![
                Term::from_field_i64(searcher.fields.album_db_id, album.id as i64),
                Term::from_field_i64(searcher.fields.disc_db_id, i64::MAX),
                Term::from_field_i64(searcher.fields.track_db_id, i64::MAX),
            ]);
            index_writer.delete_query(Box::new(query))?;
            index_writer.add_document(searcher.build_track_document(
                &album.title,
                &album.artist,
                album.id as i64,
                None,
                None,
            ))?;
            index_writer.commit().await?;
        }

        Ok(Some(AlbumInfo(album)))
    }

    /// Update basic disc information.
    async fn update_disc_info<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::UpdateDiscInfoInput,
    ) -> anyhow::Result<Option<DiscInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(model) = disc::Entity::find_by_id(input.id.parse::<i32>()?)
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let need_update_search_index = input.title.is_some() || input.artist.is_some();
        let disc: disc::ActiveModel = model.into();
        let disc = input.update(disc, db).await?;

        if need_update_search_index {
            let searcher = ctx.data::<RepositorySearchManager>().unwrap();
            let index_writer = searcher.writer().await;
            let query = PhraseQuery::new(vec![
                Term::from_field_i64(searcher.fields.album_db_id, disc.album_db_id as i64),
                Term::from_field_i64(searcher.fields.disc_db_id, disc.id as i64),
                Term::from_field_i64(searcher.fields.track_db_id, i64::MAX),
            ]);
            index_writer.delete_query(Box::new(query))?;
            index_writer.add_document(searcher.build_track_document(
                disc.title.as_deref().unwrap_or_default(),
                disc.artist.as_deref().unwrap_or_default(),
                disc.album_db_id as i64,
                Some(disc.id as i64),
                None,
            ))?;
            index_writer.commit().await?;
        }
        Ok(Some(DiscInfo(disc)))
    }

    /// Update basic track information.
    async fn update_track_info<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::UpdateTrackInfoInput,
    ) -> anyhow::Result<Option<TrackInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(model) = track::Entity::find_by_id(input.id.parse::<i32>()?)
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let need_update_search_index = input.title.is_some() || input.artist.is_some();
        let track: track::ActiveModel = model.into();
        let track = input.update(track, db).await?;

        if need_update_search_index {
            let searcher = ctx.data::<RepositorySearchManager>().unwrap();
            let index_writer = searcher.writer().await;
            let query = PhraseQuery::new(vec![
                Term::from_field_i64(searcher.fields.album_db_id, track.album_db_id as i64),
                Term::from_field_i64(searcher.fields.disc_db_id, track.disc_db_id as i64),
                Term::from_field_i64(searcher.fields.track_db_id, track.id as i64),
            ]);
            index_writer.delete_query(Box::new(query))?;
            index_writer.add_document(searcher.build_track_document(
                &track.title,
                &track.artist,
                track.album_db_id as i64,
                Some(track.disc_db_id as i64),
                Some(track.id as i64),
            ))?;
            index_writer.commit().await?;
        }
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
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let searcher = ctx.data::<RepositorySearchManager>().unwrap();
        let index_writer = searcher.writer().await;

        let Some(album) = album::Entity::find_by_id(input.id.parse::<i32>()?)
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let level = MetadataOrganizeLevel::from_str(&album.level.to_string())?;
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

        // 3. delete old indexes
        let query = TermQuery::new(
            Term::from_field_i64(searcher.fields.album_db_id, album.id as i64),
            Default::default(),
        );
        let query_not = PhraseQuery::new(vec![
            Term::from_field_i64(searcher.fields.disc_db_id, i64::MAX),
            Term::from_field_i64(searcher.fields.track_db_id, i64::MAX),
        ]);
        let query = BooleanQuery::new(vec![
            (Occur::Must, query.box_clone()),
            (Occur::MustNot, query_not.box_clone()),
        ]);
        index_writer.delete_query(Box::new(query))?;

        // 3. insert new discs and tracks
        for (index, disc) in input.discs.into_iter().enumerate() {
            disc.insert(&txn, &index_writer, album_db_id, index as i32)
                .await?;
        }

        txn.commit().await?;
        index_writer.commit().await?;

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
        let db = ctx.data::<DatabaseConnection>().unwrap();

        let Some(disc) = disc::Entity::find_by_id(input.id.parse::<i32>()?)
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let searcher = ctx.data::<RepositorySearchManager>().unwrap();
        let index_writer = searcher.writer().await;

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

        // 2. delete old indexes
        let query = PhraseQuery::new(vec![
            Term::from_field_i64(searcher.fields.album_db_id, disc.album_db_id as i64),
            Term::from_field_i64(searcher.fields.disc_db_id, disc.id as i64),
        ]);
        let query_not = TermQuery::new(
            Term::from_field_i64(searcher.fields.track_db_id, i64::MAX),
            Default::default(),
        );
        let query = BooleanQuery::new(vec![
            (Occur::Must, query.box_clone()),
            (Occur::MustNot, query_not.box_clone()),
        ]);
        index_writer.delete_query(Box::new(query))?;

        // 3. insert new tracks
        for (index, track) in input.tracks.into_iter().enumerate() {
            track
                .insert(&txn, &index_writer, album_db_id, disc_db_id, index as i32)
                .await?;
        }

        txn.commit().await?;
        index_writer.commit().await?;

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
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let Some(album) = album::Entity::find_by_id(input.id.parse::<i32>()?)
            .one(db)
            .await?
        else {
            return Ok(None);
        };

        let original_level = MetadataOrganizeLevel::from_str(&album.level.to_string())?;
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
                "Cannot increase/decrease organize level from {:?} to {:?}",
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
        album.level = ActiveValue::set(input.level.into());
        album.updated_at = ActiveValue::set(now());

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
        let db = ctx.data::<DatabaseConnection>().unwrap();

        // TODO: return if already exists
        let tag = tag_info::ActiveModel {
            name: ActiveValue::set(name),
            r#type: ActiveValue::set(r#type.into()),
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
                        disc_db_id: ActiveValue::set(Some(disc_db_id)),
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
                        disc_db_id: ActiveValue::set(Some(track.disc_db_id)),
                        track_db_id: ActiveValue::set(Some(track_db_id)),
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

    async fn rebuild_search_index(&self, ctx: &Context<'_>) -> anyhow::Result<bool> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let searcher = ctx.data::<RepositorySearchManager>().unwrap();

        // 1. clear search index
        let writer = searcher.writer().await;
        writer.delete_all()?;

        // 2. insert albums
        let albums: Vec<(i32, String, String)> = album::Entity::find()
            .select_only()
            .column(album::Column::Id)
            .column(album::Column::Title)
            .column(album::Column::Artist)
            .into_tuple()
            .all(db)
            .await?;
        for (album_db_id, title, artist) in albums {
            writer.add_document(searcher.build_track_document(
                &title,
                &artist,
                album_db_id as i64,
                None,
                None,
            ))?;
        }

        // 3. insert discs
        let discs: Vec<(i32, i32, Option<String>, Option<String>)> = disc::Entity::find()
            .select_only()
            .column(disc::Column::AlbumDbId)
            .column(disc::Column::Id)
            .column(disc::Column::Title)
            .column(disc::Column::Artist)
            .into_tuple()
            .all(db)
            .await?;
        for (album_db_id, disc_db_id, title, artist) in discs {
            writer.add_document(searcher.build_track_document(
                &title.unwrap_or_default(),
                &artist.unwrap_or_default(),
                album_db_id as i64,
                Some(disc_db_id as i64),
                None,
            ))?;
        }

        // 4. insert tracks
        let tracks: Vec<(i32, i32, i32, String, String)> = track::Entity::find()
            .select_only()
            .column(track::Column::AlbumDbId)
            .column(track::Column::DiscDbId)
            .column(track::Column::Id)
            .column(track::Column::Title)
            .column(track::Column::Artist)
            .into_tuple()
            .all(db)
            .await?;
        for (album_db_id, disc_db_id, track_db_id, title, artist) in tracks {
            writer.add_document(searcher.build_track_document(
                &title,
                &artist,
                album_db_id as i64,
                Some(disc_db_id as i64),
                Some(track_db_id as i64),
            ))?;
        }

        // 5. commit
        writer.commit().await?;

        Ok(true)
    }
}
