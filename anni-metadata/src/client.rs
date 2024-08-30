use crate::{mutation, query, schema::ID};
use cynic::{http::ReqwestExt, MutationBuilder, QueryBuilder};

pub struct AnnimClient {
    client: reqwest::Client,
    endpoint: String,
}

enum TagLocation {
    Album,
    Disc,
    Track,
}

impl AnnimClient {
    pub fn new(endpoint: String, auth: Option<&str>) -> Self {
        let mut client = reqwest::Client::builder();
        if let Some(auth) = auth {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(auth).unwrap(),
            );
            client = client.default_headers(headers);
        }

        Self {
            client: client.build().unwrap(),
            endpoint,
        }
    }

    pub async fn album(&self, album_id: uuid::Uuid) -> anyhow::Result<Option<query::album::Album>> {
        let query = query::album::AlbumQuery::build(query::album::AlbumVariables { album_id });
        let response = self.client.post(&self.endpoint).run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response.data.and_then(|data| data.album))
    }

    pub async fn add_album(
        &self,
        album: mutation::add_album::AddAlbumInput<'_>,
    ) -> anyhow::Result<Option<query::album::Album>> {
        let query =
            mutation::add_album::AddAlbumMutation::build(mutation::add_album::AddAlbumVariables {
                album,
            });
        let response = self.client.post(&self.endpoint).run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response.data.and_then(|data| data.add_album))
    }

    pub async fn tag(
        &self,
        name: String,
        tag_type: Option<query::album::TagType>,
    ) -> anyhow::Result<Vec<query::tag::Tag>> {
        let query = query::tag::TagQuery::build(query::tag::TagVariables {
            name: &name,
            type_: tag_type,
        });
        let response = self
            .client
            .post(&self.endpoint)
            .run_graphql(query)
            .await
            .unwrap();
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response.data.unwrap().tag)
    }

    pub async fn add_tag(
        &self,
        name: String,
        tag_type: query::album::TagType,
    ) -> anyhow::Result<Option<query::tag::Tag>> {
        let query = mutation::add_tag::AddTagMutation::build(mutation::add_tag::AddTagVariables {
            name: &name,
            type_: tag_type,
        });
        let response = self.client.post(&self.endpoint).run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response.data.map(|data| data.add_tag))
    }

    pub async fn set_album_tags(
        &self,
        id: &ID,
        tags: Vec<&ID>,
    ) -> anyhow::Result<query::album::Album> {
        self.set_tags(TagLocation::Album, id, tags).await
    }

    pub async fn set_disc_tags(
        &self,
        id: &ID,
        tags: Vec<&ID>,
    ) -> anyhow::Result<query::album::Album> {
        self.set_tags(TagLocation::Disc, id, tags).await
    }

    pub async fn set_track_tags(
        &self,
        id: &ID,
        tags: Vec<&ID>,
    ) -> anyhow::Result<query::album::Album> {
        self.set_tags(TagLocation::Track, id, tags).await
    }

    async fn set_tags(
        &self,
        location: TagLocation,
        id: &ID,
        tags: Vec<&ID>,
    ) -> anyhow::Result<query::album::Album> {
        let query = mutation::set_metadata_tags::SetMetadataTags::build(
            mutation::set_metadata_tags::SetMetadataTagsVariables {
                target: mutation::set_metadata_tags::MetadataIdinput {
                    album: matches!(location, TagLocation::Album).then(|| id),
                    disc: matches!(location, TagLocation::Disc).then(|| id),
                    track: matches!(location, TagLocation::Track).then(|| id),
                },
                tags,
            },
        );
        let response = self.client.post(&self.endpoint).run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response.data.unwrap().update_metadata_tags)
    }

    pub async fn add_tag_relation(
        &self,
        tag: &ID,
        parent: &ID,
    ) -> anyhow::Result<mutation::update_tag_relation::TagRelation> {
        let query = mutation::update_tag_relation::UpdateTagRelation::build(
            mutation::update_tag_relation::UpdateTagRelationVariables {
                tag,
                parent,
                remove: false,
            },
        );
        let response = self.client.post(&self.endpoint).run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response.data.unwrap().update_tag_relation.unwrap())
    }

    pub async fn remove_tag_relation(&self, tag: &ID, parent: &ID) -> anyhow::Result<()> {
        let query = mutation::update_tag_relation::UpdateTagRelation::build(
            mutation::update_tag_relation::UpdateTagRelationVariables {
                tag,
                parent,
                remove: true,
            },
        );
        let response = self.client.post(&self.endpoint).run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_album() -> anyhow::Result<()> {
        let client = AnnimClient::new("http://localhost:8000/".to_string(), Some("114514"));
        let result = client
            .album(Uuid::from_str("8da26cf7-9c9c-4209-9ed5-f5fb39e32051").unwrap())
            .await?;
        println!("{result:?}");

        Ok(())
    }
}
