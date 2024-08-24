mod input;

use async_graphql::{Context, EmptySubscription, Enum, Object, Schema, ID};
use sea_orm::{
    prelude::Uuid, ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder,
};

use crate::entities::{album, disc, track};

pub type AppSchema = Schema<MetadataQuery, MetadataMutation, EmptySubscription>;

pub fn build_schema(db: DatabaseConnection) -> AppSchema {
    Schema::build(MetadataQuery, MetadataMutation, EmptySubscription)
        .data(db)
        .finish()
}

struct AlbumInfo(album::Model);

#[Object]
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

#[Object]
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

#[Object]
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
        match self.0.r#type.as_str() {
            "normal" => TrackType::Normal,
            "instrumental" => TrackType::Instrumental,
            "absolute" => TrackType::Absolute,
            "drama" => TrackType::Drama,
            "radio" => TrackType::Radio,
            "vocal" => TrackType::Vocal,
            _ => TrackType::Unknown,
        }
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
        album_id: String,
    ) -> anyhow::Result<Option<AlbumInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let model = album::Entity::find()
            .filter(album::Column::AlbumId.contains(album_id))
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
        input: input::CreateAlbumInfoInput,
    ) -> anyhow::Result<AlbumInfo> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
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
        let model = album.insert(db).await?;
        Ok(AlbumInfo(model))
    }

    async fn update_album<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: input::UpdateAlbumInfoInput,
    ) -> anyhow::Result<Option<AlbumInfo>> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let model = match (&input.id, input.album_id) {
            (Some(id), None) => {
                album::Entity::find()
                    .filter(album::Column::Id.eq(id.parse::<i32>()?))
                    .one(db)
                    .await?
            }
            (None, Some(album_id)) => {
                album::Entity::find()
                    .filter(album::Column::AlbumId.eq(album_id))
                    .one(db)
                    .await?
            }
            _ => return Ok(None),
        }
        .unwrap();

        let album: album::ActiveModel = model.into();
        let album = input.apply(album).update(db).await?;

        Ok(Some(AlbumInfo(album)))
    }
}
