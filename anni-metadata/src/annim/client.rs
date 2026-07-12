use crate::{
    annim::{mutation, query, ID},
    model,
};
use cynic::{http::ReqwestExt, MutationBuilder, QueryBuilder};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use thiserror::Error;

use super::query::album::MetadataOrganizeLevel;

pub struct AnnimClient {
    client: Result<reqwest::Client, AnnimClientConfigurationError>,
    endpoint: String,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum AnnimClientConfigurationError {
    #[error("Annim bearer token cannot be empty")]
    EmptyBearerToken,
    #[error("Annim bearer token contains characters that are not allowed by RFC 6750")]
    InvalidBearerToken,
    #[error("failed to build the Annim HTTP client")]
    HttpClient,
}

enum TagLocation {
    Album,
    Disc,
    Track,
}

impl AnnimClient {
    pub fn new(endpoint: String, auth: Option<&str>) -> Self {
        Self {
            client: build_http_client(auth),
            endpoint,
        }
    }

    /// Creates a client and reports invalid authentication configuration immediately.
    pub fn try_new(
        endpoint: String,
        auth: Option<&str>,
    ) -> Result<Self, AnnimClientConfigurationError> {
        Ok(Self {
            client: Ok(build_http_client(auth)?),
            endpoint,
        })
    }

    fn post(&self) -> anyhow::Result<reqwest::RequestBuilder> {
        let client = self
            .client
            .as_ref()
            .map_err(|error| anyhow::Error::new(*error))?;
        Ok(client.post(&self.endpoint))
    }

    pub async fn album(
        &self,
        album_id: uuid::Uuid,
    ) -> anyhow::Result<Option<query::album::AlbumFragment>> {
        let query = query::album::AlbumQuery::build(query::album::AlbumVariables { album_id });
        let response = self.post()?.run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response.data.and_then(|data| data.album))
    }

    pub async fn albums(
        &self,
        album_ids: Vec<uuid::Uuid>,
    ) -> anyhow::Result<Vec<query::album::AlbumFragment>> {
        let query = query::albums::AlbumsQuery::build(query::albums::AlbumsVariables {
            after: None,
            first: Some(album_ids.len() as i32),
            album_ids: Some(album_ids),
        });
        let response = self.post()?.run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response
            .data
            .and_then(|data| data.albums)
            .map(|d| d.nodes)
            .unwrap())
    }

    pub async fn add_album(
        &self,
        album: &model::Album,
        commit: bool,
    ) -> anyhow::Result<query::album::AlbumFragment> {
        let discs: Vec<_> = album.iter().collect();
        let input = mutation::add_album::AddAlbumInput {
            album_id: Some(album.album_id()),
            title: album.title_raw(),
            edition: album.edition(),
            catalog: Some(album.catalog()),
            artist: album.artist(),
            year: album.release_date().year() as i32,
            month: album.release_date().month().map(|r| r as i32),
            day: album.release_date().day().map(|r| r as i32),
            extra: None,
            discs: discs
                .iter()
                .map(|disc| mutation::add_album::CreateAlbumDiscInput {
                    title: disc.title_raw(),
                    catalog: Some(disc.catalog()),
                    artist: disc.artist_raw(),
                    tracks: disc
                        .iter()
                        .map(|track| mutation::add_album::CreateAlbumTrackInput {
                            title: track.title(),
                            artist: track.artist(),
                            type_: track.track_type().into(),
                        })
                        .collect(),
                })
                .collect(),
        };

        self.add_album_input(input, commit).await
    }

    pub async fn add_album_input(
        &self,
        input: mutation::add_album::AddAlbumInput<'_>,
        commit: bool,
    ) -> anyhow::Result<query::album::AlbumFragment> {
        let query =
            mutation::add_album::AddAlbumMutation::build(mutation::add_album::AddAlbumVariables {
                album: input,
                commit: Some(commit),
            });
        let response = self.post()?.run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response.data.unwrap().add_album)
    }

    pub async fn tag(
        &self,
        name: String,
        tag_type: Option<query::album::TagTypeInput>,
    ) -> anyhow::Result<Vec<query::tag::Tag>> {
        let query = query::tag::TagQuery::build(query::tag::TagVariables {
            name: &name,
            type_: tag_type,
        });
        let response = self.post()?.run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response.data.unwrap().tag)
    }

    pub async fn add_tag(
        &self,
        name: String,
        tag_type: query::album::TagTypeInput,
    ) -> anyhow::Result<Option<query::tag::Tag>> {
        let query = mutation::add_tag::AddTagMutation::build(mutation::add_tag::AddTagVariables {
            name: &name,
            type_: tag_type,
        });
        let response = self.post()?.run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(response.data.map(|data| data.add_tag))
    }

    pub async fn set_album_tags(
        &self,
        id: &ID,
        tags: Vec<&ID>,
    ) -> anyhow::Result<query::album::AlbumFragment> {
        self.set_tags(TagLocation::Album, id, tags).await
    }

    pub async fn set_disc_tags(
        &self,
        id: &ID,
        tags: Vec<&ID>,
    ) -> anyhow::Result<query::album::AlbumFragment> {
        self.set_tags(TagLocation::Disc, id, tags).await
    }

    pub async fn set_track_tags(
        &self,
        id: &ID,
        tags: Vec<&ID>,
    ) -> anyhow::Result<query::album::AlbumFragment> {
        self.set_tags(TagLocation::Track, id, tags).await
    }

    async fn set_tags(
        &self,
        location: TagLocation,
        id: &ID,
        tags: Vec<&ID>,
    ) -> anyhow::Result<query::album::AlbumFragment> {
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
        let response = self.post()?.run_graphql(query).await?;
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
        let response = self.post()?.run_graphql(query).await?;
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
        let response = self.post()?.run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(())
    }

    pub async fn set_album_organize_level(
        &self,
        album: &ID,
        level: MetadataOrganizeLevel,
    ) -> anyhow::Result<()> {
        let query = mutation::set_organize_level::SetMetadataTags::build(
            mutation::set_organize_level::SetMetadataTagsVariables { id: album, level },
        );
        let response = self.post()?.run_graphql(query).await?;
        if let Some(errors) = response.errors {
            anyhow::bail!("GraphQL error: {:?}", errors);
        }

        Ok(())
    }
}

fn build_http_client(auth: Option<&str>) -> Result<reqwest::Client, AnnimClientConfigurationError> {
    let mut client = reqwest::Client::builder();
    if let Some(token) = auth {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, bearer_authorization_header(token)?);
        client = client.default_headers(headers);
    }

    client
        .build()
        .map_err(|_| AnnimClientConfigurationError::HttpClient)
}

fn bearer_authorization_header(token: &str) -> Result<HeaderValue, AnnimClientConfigurationError> {
    if token.is_empty() {
        return Err(AnnimClientConfigurationError::EmptyBearerToken);
    }

    let mut padding_started = false;
    for byte in token.bytes() {
        if byte == b'=' {
            padding_started = true;
            continue;
        }

        let is_token_character =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/');
        if padding_started || !is_token_character {
            return Err(AnnimClientConfigurationError::InvalidBearerToken);
        }
    }

    let mut value = HeaderValue::from_bytes(format!("Bearer {token}").as_bytes())
        .map_err(|_| AnnimClientConfigurationError::InvalidBearerToken)?;
    value.set_sensitive(true);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use uuid::Uuid;

    #[test]
    fn authorization_header_uses_bearer_scheme_and_is_sensitive() {
        let header = bearer_authorization_header("test-token_123").unwrap();

        assert_eq!(header.to_str().unwrap(), "Bearer test-token_123");
        assert!(header.is_sensitive());
    }

    #[tokio::test]
    async fn invalid_bearer_tokens_return_redacted_errors() {
        assert_eq!(
            bearer_authorization_header(""),
            Err(AnnimClientConfigurationError::EmptyBearerToken)
        );

        let token = "do-not-leak token";
        let immediate_error =
            match AnnimClient::try_new("http://127.0.0.1:9/graphql".to_owned(), Some(token)) {
                Ok(_) => panic!("an invalid bearer token must be rejected"),
                Err(error) => error,
            };
        let rendered_error = format!("{immediate_error:?}: {immediate_error}");
        assert_eq!(
            immediate_error,
            AnnimClientConfigurationError::InvalidBearerToken
        );
        assert!(!rendered_error.contains(token));

        let compatible_client =
            AnnimClient::new("http://127.0.0.1:9/graphql".to_owned(), Some(token));
        let request_error = compatible_client.album(Uuid::nil()).await.unwrap_err();
        let rendered_request_error = format!("{request_error:?}: {request_error}");
        assert!(rendered_request_error.contains("RFC 6750"));
        assert!(!rendered_request_error.contains(token));
    }

    #[tokio::test]
    #[ignore = "requires a running annim service with the fixture album"]
    async fn test_album() -> anyhow::Result<()> {
        let client = AnnimClient::new("http://localhost:8000/".to_string(), Some("114514"));
        let result = client
            .album(Uuid::from_str("8da26cf7-9c9c-4209-9ed5-f5fb39e32051").unwrap())
            .await?;
        println!("{result:?}");

        Ok(())
    }
}
