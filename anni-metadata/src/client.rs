use crate::{mutation, query};
use cynic::{http::ReqwestExt, MutationBuilder, QueryBuilder};

pub struct AnnimClient {
    client: reqwest::Client,
    endpoint: String,
}

impl AnnimClient {
    pub fn new(endpoint: String) -> Self {
        Self {
            client: reqwest::Client::new(),
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
        tag_type: Option<query::tag::TagType>,
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
        tag_type: query::tag::TagType,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_album() -> anyhow::Result<()> {
        let client = AnnimClient::new("http://localhost:8000/".to_string());
        let result = client
            .album(Uuid::from_str("8da26cf7-9c9c-4209-9ed5-f5fb39e32051").unwrap())
            .await?;
        println!("{result:?}");

        Ok(())
    }
}
