use cynic::http::ReqwestExt;
use cynic::QueryBuilder;
use query::album::{AlbumQuery, AlbumVariables};
use uuid::Uuid;

#[cynic::schema("annim")]
pub(crate) mod schema {}
pub mod query;

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

    pub async fn album(&self, album_id: Uuid) -> anyhow::Result<Option<query::album::Album>> {
        let query = AlbumQuery::build(AlbumVariables { album_id });
        let response = self.client.post(&self.endpoint).run_graphql(query).await?;
        if let Some(errors) = response.errors {
            return Err(anyhow::anyhow!("GraphQL error: {:?}", errors));
        }

        Ok(response.data.and_then(|data| data.album))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

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
