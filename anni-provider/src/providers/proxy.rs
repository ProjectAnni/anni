use std::borrow::Cow;
use std::collections::HashSet;
use crate::{AnniProvider, ProviderError, ResourceReader, AudioResourceReader, AudioInfo, Range};
use async_trait::async_trait;
use futures::TryStreamExt;
use reqwest::Response;
use crate::providers::drive::content_range_to_range;

pub struct ProxyBackend {
    url: String,
    auth: String,
    client: reqwest::Client,
}

impl ProxyBackend {
    pub fn new(url: String, auth: String) -> Self {
        Self {
            url,
            auth,
            client: reqwest::Client::new(),
        }
    }

    pub async fn get(&self, path: &str, range: &Range) -> reqwest::Result<Response> {
        let mut req = self.client.get(&format!("{}{}", self.url, path))
            .header("Authorization", &self.auth);
        if let Some(range) = range.to_range_header() {
            req = req.header("Range", range);
        }
        let req = req.build()
            .unwrap();
        self.client.execute(req).await
    }

    pub async fn head(&self, path: &str) -> reqwest::Result<Response> {
        let req = self.client.head(&format!("{}{}", self.url, path))
            .header("Authorization", &self.auth)
            .build()
            .unwrap();
        self.client.execute(req).await
    }
}

#[async_trait]
impl AnniProvider for ProxyBackend {
    async fn albums(&self) -> Result<HashSet<Cow<str>>, ProviderError> {
        let r = self.get("/albums", &Range::FULL).await.map_err(|e| ProviderError::RequestError(e))?;
        Ok(r.json().await?)
    }

    async fn get_audio_info(&self, album_id: &str, disc_id: u8, track_id: u8) -> Result<AudioInfo, ProviderError> {
        let response = self.head(&format!("/albums/{}/discs/{}/tracks/{}", album_id, disc_id, track_id)).await.map_err(|e| ProviderError::RequestError(e))?;
        audio_info_from_response(&response)
    }

    async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8, range: Range) -> Result<AudioResourceReader, ProviderError> {
        let response = self.get(&format!("/{}/{}/{}?quality=lossless", album_id, disc_id, track_id), &range).await.map_err(|e| ProviderError::RequestError(e))?;
        let info = audio_info_from_response(&response)?;

        let range = response.headers().get("Content-Range").map(|s| s.to_str().unwrap().to_string());
        let body = response
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            .into_async_read();
        let body = tokio_util::compat::FuturesAsyncReadCompatExt::compat(body);
        Ok(AudioResourceReader {
            info,
            range: content_range_to_range(range.as_deref()),
            reader: Box::pin(body),
        })
    }

    async fn get_cover(&self, album_id: &str, disc_id: Option<u8>) -> Result<ResourceReader, ProviderError> {
        let path = match disc_id {
            Some(disc_id) => format!("/{}/{}/cover", album_id, disc_id),
            None => format!("/{}/cover", album_id),
        };
        let resp = self.get(&path, &Range::FULL).await.map_err(|e| ProviderError::RequestError(e))?;
        let body = resp
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            .into_async_read();
        let body = tokio_util::compat::FuturesAsyncReadCompatExt::compat(body);
        Ok(Box::pin(body))
    }

    async fn reload(&mut self) -> Result<(), ProviderError> {
        // proxy provider does not need to be reloaded
        Ok(())
    }
}

fn audio_info_from_response(response: &Response) -> Result<AudioInfo, ProviderError> {
    let original_size = match response.headers().get("x-origin-size") {
        Some(s) => s.to_str().unwrap_or("0"),
        None => "0",
    }.to_string();
    let duration = match response.headers().get("x-duration-seconds") {
        Some(s) => s.to_str().unwrap_or("0"),
        None => "0",
    }.to_string();
    let extension = match response.headers().get("Content-Type") {
        Some(content_type) => {
            let content_type = content_type.to_str().unwrap_or("audio/flac");
            content_type.strip_prefix("audio/").unwrap_or("flac").to_string()
        }
        None => "flac".to_string(),
    };
    Ok(AudioInfo {
        extension,
        size: original_size.parse().map_err(|_| ProviderError::GeneralError)?,
        duration: duration.parse().map_err(|_| ProviderError::GeneralError)?,
    })
}