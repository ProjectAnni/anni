use std::borrow::Cow;
use std::collections::HashSet;
use std::str::FromStr;
use crate::{AnniProvider, ProviderError, ResourceReader, AudioResourceReader};
use async_trait::async_trait;
use futures::TryStreamExt;
use reqwest::Response;

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

    pub async fn get(&self, path: &str, range: Option<String>) -> reqwest::Result<Response> {
        let mut req = self.client.get(&format!("{}{}", self.url, path))
            .header("Authorization", &self.auth);
        if let Some(range) = range {
            req = req.header("Range", range);
        }
        let req = req.build()
            .unwrap();
        self.client.execute(req).await
    }
}

#[async_trait]
impl AnniProvider for ProxyBackend {
    async fn albums(&self) -> Result<HashSet<Cow<str>>, ProviderError> {
        let r = self.get("/albums", None).await.map_err(|e| ProviderError::RequestError(e))?;
        Ok(r.json().await?)
    }

    async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8, range: Option<String>) -> Result<AudioResourceReader, ProviderError> {
        let resp = self.get(&format!("/{}/{}/{}?prefer_bitrate=lossless", album_id, disc_id, track_id), range).await.map_err(|e| ProviderError::RequestError(e))?;
        let original_size = match resp.headers().get("x-origin-size") {
            Some(s) => s.to_str().unwrap_or("0"),
            None => "0",
        }.to_string();
        let duration = match resp.headers().get("x-duration-seconds") {
            Some(s) => s.to_str().unwrap_or("0"),
            None => "0",
        }.to_string();
        let extension = match resp.headers().get("Content-Type") {
            Some(content_type) => {
                let content_type = content_type.to_str().unwrap_or("audio/flac");
                content_type.strip_prefix("audio/").unwrap_or("flac").to_string()
            }
            None => "flac".to_string(),
        };
        let range = resp.headers().get("Content-Range").map(|s| s.to_str().unwrap().to_string());
        let body = resp
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            .into_async_read();
        let body = tokio_util::compat::FuturesAsyncReadCompatExt::compat(body);
        Ok(AudioResourceReader {
            extension,
            size: usize::from_str(original_size.as_str()).unwrap(),
            duration: u64::from_str(duration.as_str()).unwrap(),
            range,
            reader: Box::pin(body),
        })
    }

    async fn get_cover(&self, album_id: &str, disc_id: Option<u8>) -> Result<ResourceReader, ProviderError> {
        let path = match disc_id {
            Some(disc_id) => format!("/{}/{}/cover", album_id, disc_id),
            None => format!("/{}/cover", album_id),
        };
        let resp = self.get(&path, None).await.map_err(|e| ProviderError::RequestError(e))?;
        let body = resp
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            .into_async_read();
        let body = tokio_util::compat::FuturesAsyncReadCompatExt::compat(body);
        Ok(Box::pin(body))
    }

    async fn reload(&mut self) -> Result<(), ProviderError> {
        // proxy backend does not need to be reloaded
        Ok(())
    }
}