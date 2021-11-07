use std::collections::HashSet;
use std::str::FromStr;
use crate::{Backend, BackendError, BackendReader, BackendReaderExt};
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

    pub async fn get(&self, path: &str) -> reqwest::Result<Response> {
        let req = self.client.get(&format!("{}{}", self.url, path))
            .header("Authorization", &self.auth)
            .build()
            .unwrap();
        self.client.execute(req).await
    }
}

#[async_trait]
impl Backend for ProxyBackend {
    async fn albums(&mut self) -> Result<HashSet<String>, BackendError> {
        let r = self.get("/albums").await.map_err(|e| BackendError::RequestError(e))?;
        Ok(r.json().await?)
    }

    async fn get_audio(&self, catalog: &str, track_id: u8) -> Result<BackendReaderExt, BackendError> {
        let resp = self.get(&format!("/{}/{}?prefer_bitrate=loseless", catalog, track_id)).await.map_err(|e| BackendError::RequestError(e))?;
        let original_size = match resp.headers().get("x-origin-size") {
            Some(s) => s.to_str().unwrap_or("0"),
            None => "0",
        }.to_string();
        let body = resp
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            .into_async_read();
        let body = tokio_util::compat::FuturesAsyncReadCompatExt::compat(body);
        Ok(BackendReaderExt {
            // FIXME: the correct extension might not be `flac`
            extension: "flac".to_string(),
            // TODO: Try to get correct size from response
            size: usize::from_str(original_size.as_str()).unwrap(),
            reader: Box::pin(body),
        })
    }

    async fn get_cover(&self, catalog: &str) -> Result<BackendReader, BackendError> {
        let resp = self.get(&format!("/{}/cover", catalog)).await.map_err(|e| BackendError::RequestError(e))?;
        let body = resp
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            .into_async_read();
        let body = tokio_util::compat::FuturesAsyncReadCompatExt::compat(body);
        Ok(Box::pin(body))
    }
}