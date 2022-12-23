use std::{collections::HashMap, ops::Deref};

use anni_repo::prelude::*;
use anni_repo::OwnedRepositoryManager;
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
        self.0.iter().map(DiscInfo).collect()
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
        self.0.tags_iter().map(TagInfo).collect()
    }

    // we do not provide disc type because it's useless

    async fn tracks<'disc>(&'disc self) -> Vec<TrackInfo<'album, 'disc>> {
        self.0.iter().map(TrackInfo).collect()
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
        self.0.tags_iter().map(TagInfo).collect()
    }
}

struct TagInfo<'tag>(&'tag TagRef<'tag>);

#[Object]
impl<'tag> TagInfo<'tag> {
    async fn name(&self) -> &str {
        self.0.name()
    }

    #[graphql(name = "type")]
    async fn tag_type(&self) -> &str {
        self.0.tag_type().as_ref()
    }

    async fn includes<'ctx>(&'tag self, ctx: &Context<'ctx>) -> Vec<TagInfo<'tag>>
    where
        'ctx: 'tag,
    {
        let manager = ctx.data_unchecked::<OwnedRepositoryManager>();
        manager
            .child_tags(self.0)
            .into_iter()
            .map(TagInfo)
            .collect()
    }

    #[graphql(flatten)]
    async fn detail<'ctx>(&self, ctx: &Context<'ctx>) -> TagDetail<'tag>
    where
        'ctx: 'tag,
    {
        let manager = ctx.data_unchecked::<OwnedRepositoryManager>();
        TagDetail(manager.tag(self.0).unwrap())
    }
}

struct TagDetail<'tag>(&'tag Tag);

#[Object]
impl<'tag> TagDetail<'tag> {
    async fn names(&self) -> &HashMap<String, String> {
        self.0.names()
    }

    async fn included_by(&self) -> Vec<TagInfo> {
        self.0
            .parents()
            .iter()
            .map(|t| TagInfo(t.deref()))
            .collect()
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
        Ok(manager.album(&album_id.parse()?).map(AlbumInfo))
    }

    async fn albums_by_tag<'ctx>(&self, ctx: &Context<'ctx>, tag: String) -> Vec<AlbumInfo<'ctx>> {
        let tag = TagRef::from_cow_str(tag);
        let manager = ctx.data_unchecked::<OwnedRepositoryManager>();
        manager
            .albums_tagged_by(&tag)
            .map(|ids| {
                ids.iter()
                    .map(|album_id| manager.album(album_id).unwrap())
                    .map(AlbumInfo)
                    .collect()
            })
            .unwrap_or_else(|| Vec::new())
    }
}
