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

    pub async fn album(&self, album_id: Uuid) {
        let query = AlbumQuery::build(AlbumVariables { album_id });
        let response = self
            .client
            .post(&self.endpoint)
            .run_graphql(query)
            .await
            .unwrap();

        let album = response.data.unwrap().album.unwrap();
        println!("{:?}", album);
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[tokio::test]
    async fn test_album() {
        let client = AnnimClient::new("http://localhost:8000/".to_string());
        client
            .album(Uuid::from_str("cee3c2fa-14b4-422b-ab0d-290c4f0020f4").unwrap())
            .await;
    }
}
