use std::{collections::HashMap, ops::Deref};

use anni_repo::{
    prelude::{Album, DiscRef, TagRef, TrackRef},
    OwnedRepositoryManager,
};
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};

pub type AppSchema = Schema<MetadataQuery, EmptyMutation, EmptySubscription>;

pub fn build_schema(manager: OwnedRepositoryManager) -> AppSchema {
    Schema::build(MetadataQuery, EmptyMutation, EmptySubscription)
        .data(manager)
        .finish()
}

struct AlbumInfo<'album>(&'album Album);

#[Object]
impl<'album> AlbumInfo<'album> {
    async fn album_id(&self) -> String {
        self.0.album_id.to_string()
    }

    async fn title(&self) -> &str {
        self.0.title_raw()
    }

    async fn edition(&self) -> Option<&str> {
        self.0.edition()
    }

    async fn catalog(&self) -> &str {
        self.0.catalog()
    }

    async fn artist(&self) -> &str {
        self.0.artist()
    }

    async fn artists(&self) -> Option<&HashMap<String, String>> {
        self.0.artists.as_ref()
    }

    #[graphql(name = "date")]
    async fn release_date(&self) -> String {
        self.0.release_date().to_string()
    }

    async fn tags(&self) -> Vec<TagInfo<'album>> {
        self.0.tags.iter().map(|t| TagInfo(t.deref())).collect()
    }

    // we do not provide album type because it's useless

    async fn discs(&self) -> Vec<DiscInfo> {
        self.0.iter().map(|d| DiscInfo(d)).collect()
    }
}

struct DiscInfo<'album>(DiscRef<'album>);

#[Object]
impl<'album> DiscInfo<'album> {
    async fn title(&self) -> Option<&str> {
        self.0.title_raw()
    }

    async fn catalog(&self) -> &str {
        self.0.catalog()
    }

    async fn artist(&self) -> Option<&str> {
        self.0.artist_raw()
    }

    async fn artists(&self) -> Option<&HashMap<String, String>> {
        self.0.artists()
    }

    async fn tags(&self) -> Vec<TagInfo> {
        self.0.tags_iter().map(|t| TagInfo(t)).collect()
    }

    // we do not provide disc type because it's useless

    async fn tracks<'disc>(&'disc self) -> Vec<TrackInfo<'album, 'disc>> {
        self.0.iter().map(|t| TrackInfo(t)).collect()
    }
}

struct TrackInfo<'album, 'disc>(TrackRef<'album, 'disc>);

#[Object]
impl TrackInfo<'_, '_> {
    async fn title(&self) -> &str {
        self.0.title()
    }

    async fn artist(&self) -> &str {
        self.0.artist()
    }

    #[graphql(name = "type")]
    async fn track_type(&self) -> &str {
        self.0.track_type().as_ref()
    }

    async fn artists(&self) -> Option<&HashMap<String, String>> {
        self.0.artists()
    }

    async fn tags(&self) -> Vec<TagInfo> {
        self.0.tags_iter().map(|t| TagInfo(t)).collect()
    }
}

struct TagInfo<'tag>(&'tag TagRef<'tag>);

#[Object]
impl TagInfo<'_> {
    async fn name(&self) -> &str {
        self.0.name()
    }

    #[graphql(name = "type")]
    async fn tag_type(&self) -> &str {
        self.0.tag_type().as_ref()
    }
}

pub struct MetadataQuery;

#[Object]
impl MetadataQuery {
    async fn album<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        album_id: String,
    ) -> anyhow::Result<Option<AlbumInfo<'ctx>>> {
        let manager = ctx.data_unchecked::<OwnedRepositoryManager>();
        Ok(manager
            .album(&album_id.parse()?)
            .map(|album| AlbumInfo(album)))
    }
}
