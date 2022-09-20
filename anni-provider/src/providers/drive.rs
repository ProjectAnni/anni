use crate::{AnniProvider, AudioInfo, AudioResourceReader, ProviderError, Range, ResourceReader};
use async_trait::async_trait;
use google_drive3::{
    hyper, hyper::client::HttpConnector, hyper_rustls::HttpsConnector, oauth2, DriveHub,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use self::oauth2::authenticator::Authenticator;
use self::oauth2::authenticator_delegate::DefaultInstalledFlowDelegate;
use crate::utils::read_duration;
use anni_repo::db::RepoDatabaseRead;
use anni_repo::library::{AlbumFolderInfo, DiscFolderInfo};
use dashmap::DashMap;
use futures::TryStreamExt;
use google_drive3::api::{FileList, FileListCall};
use google_drive3::hyper_rustls::HttpsConnectorBuilder;
use parking_lot::Mutex;
use std::str::FromStr;
use tokio::sync::Semaphore;

pub enum DriveAuth {
    InstalledFlow {
        client_id: String,
        client_secret: String,
        project_id: Option<String>,
    },
    ServiceAccount(oauth2::ServiceAccountKey),
}

impl Default for DriveAuth {
    fn default() -> Self {
        DriveAuth::InstalledFlow {
            client_id: "175511611598-ot9agsmf6v3lf1jc3qbsf1vcru7saop7.apps.googleusercontent.com"
                .to_string(),
            client_secret: "mW1neo-JSSwzYz5Syqiiset1".to_string(),
            project_id: Some("anni-provider".to_string()),
        }
    }
}

impl DriveAuth {
    pub async fn build<P>(
        self,
        persist_path: P,
    ) -> std::io::Result<Authenticator<HttpsConnector<HttpConnector>>>
    where
        P: AsRef<Path>,
    {
        match self {
            DriveAuth::InstalledFlow {
                client_id,
                client_secret,
                project_id,
            } => {
                oauth2::InstalledFlowAuthenticator::builder(
                    oauth2::ApplicationSecret {
                        client_id,
                        project_id,
                        auth_uri: "https://accounts.google.com/o/oauth2/auth".to_string(),
                        token_uri: "https://oauth2.googleapis.com/token".to_string(),
                        auth_provider_x509_cert_url: Some(
                            "https://www.googleapis.com/oauth2/v1/certs".to_string(),
                        ),
                        client_secret,
                        redirect_uris: vec!["urn:ietf:wg:oauth:2.0:oob".to_string()],
                        client_email: None,
                        client_x509_cert_url: None,
                    },
                    oauth2::InstalledFlowReturnMethod::Interactive,
                )
                .persist_tokens_to_disk(persist_path.as_ref())
                .flow_delegate(Box::new(DefaultInstalledFlowDelegate))
                .build()
                .await
            }
            DriveAuth::ServiceAccount(sa) => {
                oauth2::ServiceAccountAuthenticator::builder(sa)
                    .persist_tokens_to_disk(persist_path.as_ref())
                    .build()
                    .await
            }
        }
    }
}

pub struct DriveProviderSettings {
    pub corpora: String,
    pub drive_id: Option<String>,
    pub token_path: PathBuf,
}

pub struct DriveClient {
    hub: Box<DriveHub<HttpsConnector<HttpConnector>>>,
    settings: DriveProviderSettings,
    /// Semaphore for rate limiting
    semaphore: Semaphore,

    // parent_id <-> file_id
    covers: DashMap<String, String>,
}

impl DriveClient {
    pub async fn new(
        auth: DriveAuth,
        settings: DriveProviderSettings,
    ) -> Result<Self, ProviderError> {
        let auth = auth.build(&settings.token_path).await?;
        auth.token(&[
            "https://www.googleapis.com/auth/drive.metadata.readonly",
            "https://www.googleapis.com/auth/drive.readonly",
        ])
        .await?;
        let hub = DriveHub::new(
            hyper::Client::builder().build(
                HttpsConnectorBuilder::new()
                    .with_native_roots()
                    .https_or_http()
                    .enable_http1()
                    .enable_http2()
                    .build(),
            ),
            auth,
        );
        Ok(Self {
            hub: Box::new(hub),
            settings,
            covers: DashMap::new(),
            semaphore: Semaphore::new(200),
        })
    }

    fn prepare_list(&self) -> FileListCall<HttpsConnector<HttpConnector>> {
        let result = self
            .hub
            .files()
            .list()
            .corpora(&self.settings.corpora)
            .supports_all_drives(true)
            .include_items_from_all_drives(true)
            .page_size(500);
        match &self.settings.drive_id {
            Some(drive_id) => result.drive_id(drive_id),
            None => result,
        }
    }

    async fn list_folder(&self, parent_id: &str) -> Result<FileList, ProviderError> {
        let permit = self.semaphore.acquire().await.unwrap();
        let (_, list) = self.prepare_list()
            .q(&format!("mimeType = 'application/vnd.google-apps.folder' and trashed = false and '{}' in parents", parent_id))
            .param("fields", "nextPageToken, files(id,name)")
            .doit().await?;
        drop(permit);
        Ok(list)
    }

    async fn get_file(
        &self,
        file_id: &str,
        range: &Range,
    ) -> Result<(ResourceReader, Range), ProviderError> {
        let permit = self.semaphore.acquire().await.unwrap();
        let (resp, _) = self
            .hub
            .files()
            .get(file_id)
            .supports_all_drives(true)
            .acknowledge_abuse(true)
            .param("alt", "media")
            .range(range.to_range_header())
            .doit()
            .await?;
        drop(permit);
        let content_range = resp
            .headers()
            .get("Content-Range")
            .map(|v| v.to_str().unwrap().to_string());
        let body = resp
            .into_body()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Error!"))
            .into_async_read();
        let body = tokio_util::compat::FuturesAsyncReadCompatExt::compat(body);
        Ok((
            Box::pin(body),
            content_range_to_range(content_range.as_deref()),
        ))
    }

    async fn get_cover_id_in(&self, parent_id: &str) -> Result<String, ProviderError> {
        if self.covers.contains_key(parent_id) {
            return self
                .covers
                .get(parent_id)
                .map(|v| v.to_string())
                .ok_or(ProviderError::FileNotFound);
        }

        let permit = self.semaphore.acquire().await.unwrap();
        let (_, list) = self.prepare_list()
            .q(&format!("trashed = false and mimeType = 'image/jpeg' and name = 'cover.jpg' and '{}' in parents", parent_id))
            .param("fields", "nextPageToken, files(id,name)")
            .doit().await?;
        drop(permit);

        let files = list.files.unwrap();
        let file = files.get(0).ok_or(ProviderError::FileNotFound)?;
        let id = file.id.as_ref().unwrap().to_string();
        self.covers.insert(parent_id.to_string(), id.clone());
        Ok(id)
    }
}

pub struct DriveProvider {
    /// Google Drive API Client
    client: DriveClient,
    /// HashMap mapping album_id and folder_id
    folders: HashMap<String, String>,
    /// Cache for mapping album_id and its discs if multiple discs exists
    /// All albums with multiple discs must be in this map
    /// If the value is None, it means the album is not cached
    /// If the value is Some, then the value of index is the folder_id of the disc
    discs: DashMap<String, Option<Vec<String>>>,
    /// Cache file id
    /// "{album_id}/cover" <-> file_id
    /// "{album_id}/{disc_id}/cover" <-> file_id
    /// "{album_id}/{disc_id}/track_id" <-> file_id
    files: DashMap<String, String>,
    /// file_id <-> (extension, filesize)
    audios: DashMap<String, (String, usize)>,

    // properties
    strict: bool,
    repo: Mutex<Option<RepoDatabaseRead>>,
}

impl DriveProvider {
    pub async fn new(
        auth: DriveAuth,
        settings: DriveProviderSettings,
        repo: Option<RepoDatabaseRead>,
    ) -> Result<Self, ProviderError> {
        let mut this = Self {
            client: DriveClient::new(auth, settings).await?,
            folders: Default::default(),
            discs: Default::default(),
            files: Default::default(),
            audios: Default::default(),
            strict: repo.is_none(),
            repo: Mutex::new(repo),
        };
        this.reload().await?;
        Ok(this)
    }

    async fn cache_discs(&self, album_id: &str) -> Result<(), ProviderError> {
        if self.folders.contains_key(album_id)
            && self.discs.contains_key(album_id)
            && self.discs.get(album_id).unwrap().is_none()
        {
            let list = self.client.list_folder(&self.folders[album_id]).await?;
            let mut discs: Vec<_> = list
                .files
                .unwrap()
                .iter()
                .filter_map(|file| {
                    let file_id = file.id.as_deref().unwrap().to_string();
                    return if self.strict {
                        let disc_index: usize = file.name.as_ref().unwrap().parse().ok()?;
                        Some((disc_index, file_id))
                    } else {
                        let DiscFolderInfo { disc_id, .. } =
                            DiscFolderInfo::from_str(file.name.as_deref().unwrap()).ok()?;
                        Some((disc_id, file_id))
                    };
                })
                .collect();
            discs.sort();
            self.discs.insert(
                album_id.to_string(),
                Some(discs.into_iter().map(|(_, id)| id).collect()),
            );
        }

        Ok(())
    }

    fn get_parent_folder(&self, album_id: &str, disc_id: Option<u8>) -> Cow<str> {
        match disc_id {
            Some(disc_id) => {
                if self.discs.contains_key(album_id) {
                    Cow::Owned(
                        self.discs.get(album_id).unwrap().as_deref().unwrap()
                            [(disc_id - 1) as usize]
                            .clone(),
                    )
                } else {
                    Cow::Borrowed(&self.folders[album_id])
                }
            }
            None => Cow::Borrowed(&self.folders[album_id]),
        }
    }
}

#[async_trait]
impl AnniProvider for DriveProvider {
    async fn albums(&self) -> Result<HashSet<Cow<str>>, ProviderError> {
        Ok(self
            .folders
            .keys()
            .map(|a| Cow::Borrowed(a.as_str()))
            .collect())
    }

    async fn get_audio(
        &self,
        album_id: &str,
        disc_id: u8,
        track_id: u8,
        range: Range,
    ) -> Result<AudioResourceReader, ProviderError> {
        // catalog not found
        if !self.folders.contains_key(album_id) {
            return Err(ProviderError::FileNotFound);
        }

        let key = format!("{album_id}/{disc_id}/{track_id}");
        if !self.files.contains_key(&key) {
            // get folder_id
            self.cache_discs(album_id).await?;
            let folder_id = self.get_parent_folder(album_id, Some(disc_id));

            // get audio file id
            let permit = self.client.semaphore.acquire().await.unwrap();
            let q = if self.strict {
                format!("trashed = false and name = '{track_id}.flac' and '{folder_id}' in parents")
            } else {
                format!("trashed = false and name contains '{track_id:02}.' and '{folder_id}' in parents")
            };
            let (_, list) = self
                .client
                .prepare_list()
                .q(&q)
                .param("fields", "nextPageToken, files(id,name,fileExtension,size)")
                .doit()
                .await?;
            drop(permit);

            let files = list.files.unwrap();
            let id = if self.strict {
                Some(files.first().ok_or_else(|| ProviderError::FileNotFound)?)
            } else {
                files.iter().reduce(|a, b| {
                    if a.name
                        .as_ref()
                        .unwrap()
                        .starts_with(&format!("{:02}.", track_id))
                    {
                        a
                    } else {
                        b
                    }
                })
            };
            if let Some(file) = id {
                let id = file.id.as_ref().unwrap();
                self.audios.insert(
                    id.to_string(),
                    (
                        file.file_extension.as_ref().unwrap().to_string(),
                        usize::from_str(file.size.as_ref().unwrap()).unwrap(),
                    ),
                );
                self.files.insert(key.to_string(), id.to_string());
            } else {
                return Err(ProviderError::FileNotFound);
            }
        }

        match self.files.get(&key) {
            Some(id) => {
                let file_id = id.value().to_string();
                drop(id); // drop lock immediately
                let metadata = self.audios.get(&file_id).unwrap().value().clone(); // drop lock inline

                let (reader, range) = self.client.get_file(&file_id, &range).await?;
                let (duration, reader) = read_duration(reader, range).await?;
                Ok(AudioResourceReader {
                    info: AudioInfo {
                        extension: metadata.0,
                        size: metadata.1,
                        duration,
                    },
                    range,
                    reader,
                })
            }
            None => Err(ProviderError::FileNotFound),
        }
    }

    async fn get_cover(
        &self,
        album_id: &str,
        disc_id: Option<u8>,
    ) -> Result<ResourceReader, ProviderError> {
        // album_id not found
        if !self.folders.contains_key(album_id) ||
            // disc not found
            (disc_id.is_some() && !matches!(disc_id, Some(1)) && !self.discs.contains_key(album_id))
        {
            return Err(ProviderError::FileNotFound);
        }

        let key = match disc_id {
            Some(disc_id) => format!("{album_id}/{disc_id}/cover"),
            None => format!("{album_id}/cover"),
        };
        let id = match self.files.get(&key) {
            Some(id) => id.to_string(),
            None => {
                // get folder_id
                self.cache_discs(album_id).await?;
                let folder_id = self.get_parent_folder(album_id, disc_id);

                // get cover file id
                self.client.get_cover_id_in(&folder_id).await?
            }
        };

        Ok(self.client.get_file(&id, &Range::FULL).await?.0)
    }

    async fn reload(&mut self) -> Result<(), ProviderError> {
        self.folders.clear();
        self.discs.clear();
        self.files.clear();
        self.audios.clear();

        if let Some(repo) = &mut *self.repo.lock() {
            repo.reload()?;
        }

        let mut page_token = String::new();
        loop {
            let permit = self.client.semaphore.acquire().await.unwrap();
            let (_, list) = self
                .client
                .prepare_list()
                .page_token(&page_token)
                .q(if self.strict {
                    "mimeType = 'application/vnd.google-apps.folder' and name != '0' and name != '1' and name != '2' and name != '3' and name != '4' and name != '5' and name != '6' and name != '7' and name != '8' and name != '9' and trashed = false"
                } else {
                    "mimeType = 'application/vnd.google-apps.folder' and trashed = false"
                })
                .param("fields", "nextPageToken, files(id,name)")
                .page_size(1000)
                .doit()
                .await?;
            drop(permit);
            for file in list.files.unwrap() {
                let name = file.name.unwrap();
                if self.strict {
                    if name.len() != 36 {
                        continue;
                    }
                    self.folders.insert(name.to_string(), file.id.unwrap());
                    self.discs.insert(name, None);
                } else {
                    if let Ok(AlbumFolderInfo {
                        release_date,
                        catalog,
                        title,
                        edition,
                        disc_count,
                    }) = AlbumFolderInfo::from_str(&name)
                    {
                        let album_id = self.repo.lock().as_ref().unwrap().match_album(
                            &catalog,
                            &release_date,
                            disc_count as u8,
                            &title,
                            edition.as_deref(),
                        )?;
                        match album_id {
                            Some(album_id) => {
                                self.folders.insert(album_id.to_string(), file.id.unwrap());
                                if disc_count > 1 {
                                    self.discs.insert(album_id.to_string(), None);
                                }
                            }
                            None => {
                                log::warn!("Album ID not found for {}, ignoring...", catalog);
                            }
                        }
                    };
                }
            }
            if list.next_page_token.is_none() {
                break;
            } else {
                page_token = list.next_page_token.unwrap();
            }
        }
        Ok(())
    }
}

pub(crate) fn content_range_to_range(content_range: Option<&str>) -> Range {
    match content_range {
        Some(content_range) => {
            // if content range header is invalid, return the full range
            if content_range.len() <= 6 {
                return Range::FULL;
            }

            // else, parse the range
            // Content-Range: bytes 0-1023/10240
            //                      | offset = 6
            let content_range = &content_range[6..];
            let (from, content_range) =
                content_range.split_once('-').unwrap_or((content_range, ""));
            let (to, total) = content_range.split_once('/').unwrap_or((content_range, ""));

            Range {
                start: from.parse().unwrap_or(0),
                end: to.parse().ok(),
                total: total.parse().ok(),
            }
        }
        None => Range::FULL,
    }
}
