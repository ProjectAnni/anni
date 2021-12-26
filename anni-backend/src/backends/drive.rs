use std::borrow::Cow;
use crate::{Backend, BackendError, BackendReaderExt, BackendReader};
use std::collections::{HashSet, HashMap};
use async_trait::async_trait;
use google_drive3::DriveHub;
use hyper_rustls::HttpsConnector;
use hyper::client::HttpConnector;

extern crate yup_oauth2 as oauth2;

use self::oauth2::authenticator::Authenticator;
use self::oauth2::authenticator_delegate::DefaultInstalledFlowDelegate;
use anni_repo::library::{album_info, disc_info};
use futures::TryStreamExt;
use google_drive3::api::FileListCall;
use std::str::FromStr;
use dashmap::DashMap;
use tokio::sync::Semaphore;
use anni_repo::db::RepoDatabaseRead;

pub enum DriveAuth {
    InstalledFlow { client_id: String, client_secret: String, project_id: Option<String> },
    ServiceAccount(oauth2::ServiceAccountKey),
}

impl Default for DriveAuth {
    fn default() -> Self {
        DriveAuth::InstalledFlow {
            client_id: "175511611598-ot9agsmf6v3lf1jc3qbsf1vcru7saop7.apps.googleusercontent.com".to_string(),
            client_secret: "mW1neo-JSSwzYz5Syqiiset1".to_string(),
            project_id: Some("project-anni".to_string()),
        }
    }
}

impl DriveAuth {
    pub async fn build(self, persist_path: &str) -> std::io::Result<Authenticator<HttpsConnector<HttpConnector>>> {
        match self {
            DriveAuth::InstalledFlow { client_id, client_secret, project_id } => {
                oauth2::InstalledFlowAuthenticator::builder(oauth2::ApplicationSecret {
                    client_id,
                    project_id,
                    auth_uri: "https://accounts.google.com/o/oauth2/auth".to_string(),
                    token_uri: "https://oauth2.googleapis.com/token".to_string(),
                    auth_provider_x509_cert_url: Some("https://www.googleapis.com/oauth2/v1/certs".to_string()),
                    client_secret,
                    redirect_uris: vec!["urn:ietf:wg:oauth:2.0:oob".to_string()],
                    client_email: None,
                    client_x509_cert_url: None,
                }, oauth2::InstalledFlowReturnMethod::Interactive)
                    .persist_tokens_to_disk(persist_path)
                    .flow_delegate(Box::new(DefaultInstalledFlowDelegate))
                    .build().await
            }
            DriveAuth::ServiceAccount(sa) => {
                oauth2::ServiceAccountAuthenticator::builder(sa).persist_tokens_to_disk(persist_path).build().await
            }
        }
    }
}

pub struct DriveBackendSettings {
    pub corpora: String,
    pub drive_id: Option<String>,
    pub token_path: String,
}

pub struct DriveBackend {
    /// Google Drive API Hub
    hub: Box<DriveHub>,
    /// HashMap mapping album_id and folder_id
    folders: HashMap<String, String>,
    /// Cache for mapping album_id and its discs if multiple discs exists
    /// All albums with multiple discs must be in this map
    /// If the value is None, it means the album is not cached
    /// If the value is Some, then the value of index is the folder_id of the disc
    discs: DashMap<String, Option<Vec<String>>>,
    /// Settings
    settings: DriveBackendSettings,
    repo: RepoDatabaseRead,
    /// Semaphore for rate limiting
    semaphore: Semaphore,
}

impl DriveBackend {
    pub async fn new(auth: DriveAuth, settings: DriveBackendSettings, repo: RepoDatabaseRead) -> Result<Self, BackendError> {
        let auth = auth.build(&settings.token_path).await?;
        auth.token(&[
            "https://www.googleapis.com/auth/drive.metadata.readonly",
            "https://www.googleapis.com/auth/drive.readonly",
        ]).await?;
        let hub = DriveHub::new(hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()), auth);
        Ok(Self {
            hub: Box::new(hub),
            folders: Default::default(),
            discs: Default::default(),
            settings,
            repo,
            semaphore: Semaphore::new(100),
        })
    }

    fn prepare_list(&self) -> FileListCall {
        let result = self.hub.files().list()
            .corpora(&self.settings.corpora)
            .supports_all_drives(true)
            .include_items_from_all_drives(true)
            .page_size(500);
        match &self.settings.drive_id {
            Some(drive_id) => result.drive_id(drive_id),
            None => result,
        }
    }

    async fn cache_discs(&self, album_id: &str) -> Result<(), BackendError> {
        if self.folders.contains_key(album_id) && self.discs.contains_key(album_id) && self.discs.get(album_id).unwrap().is_none() {
            let permit = self.semaphore.acquire().await.unwrap();
            let (_, list) = self.prepare_list()
                .q(&format!("mimeType = 'application/vnd.google-apps.folder' and trashed = false and '{}' in parents", self.folders[album_id]))
                .param("fields", "nextPageToken, files(id,name)")
                .doit().await?;
            drop(permit);
            let mut discs: Vec<_> = list.files.unwrap().iter().filter_map(|file| {
                let (_, _, disc_index) = disc_info(file.name.as_deref().unwrap()).ok()?;
                return Some((disc_index, file.id.as_deref().unwrap().to_string()));
            }).collect();
            discs.sort();
            // discs.sorted_by_key(|(disc_index, _)| *disc_index).collect();
            self.discs.insert(album_id.to_string(), Some(discs.into_iter().map(|(_, id)| id).collect()));
        }

        Ok(())
    }

    fn get_parent_folder(&self, album_id: &str, disc_id: Option<u8>) -> Cow<str> {
        match disc_id {
            Some(disc_id) => {
                if self.discs.contains_key(album_id) {
                    Cow::Owned(self.discs.get(album_id).unwrap().as_deref().unwrap()[(disc_id - 1) as usize].clone())
                } else {
                    Cow::Borrowed(&self.folders[album_id])
                }
            }
            None => Cow::Borrowed(&self.folders[album_id]),
        }
    }

    async fn get_file(&self, file_id: &str) -> Result<BackendReader, BackendError> {
        let permit = self.semaphore.acquire().await.unwrap();
        let (resp, _) = self.hub.files().get(file_id)
            .supports_all_drives(true)
            .param("alt", "media")
            .doit().await?;
        drop(permit);
        let body = resp.into_body()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Error!"))
            .into_async_read();
        let body = tokio_util::compat::FuturesAsyncReadCompatExt::compat(body);
        Ok(Box::pin(body))
    }
}

#[async_trait]
impl Backend for DriveBackend {
    async fn albums(&mut self) -> Result<HashSet<String>, BackendError> {
        self.folders.clear();
        self.discs.clear();
        self.repo.reload().await?;

        let mut page_token = String::new();
        loop {
            let permit = self.semaphore.acquire().await.unwrap();
            let (_, list) = self.prepare_list()
                .page_token(&page_token)
                .q("mimeType = 'application/vnd.google-apps.folder' and trashed = false")
                .param("fields", "nextPageToken, files(id,name)")
                .doit().await?;
            drop(permit);
            for file in list.files.unwrap() {
                let name = file.name.unwrap();
                if let Ok((release_date, catalog, title, disc_count)) = album_info(&name) {
                    let album_id = self.repo.match_album(&catalog, &release_date, disc_count as u8, &title).await?;
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
            if list.next_page_token.is_none() {
                break;
            } else {
                page_token = list.next_page_token.unwrap();
            }
        }
        Ok(self
            .folders
            .keys()
            .into_iter()
            .map(|a| a.to_owned())
            .collect())
    }

    async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8) -> Result<BackendReaderExt, BackendError> {
        // catalog not found
        if !self.folders.contains_key(album_id) {
            return Err(BackendError::FileNotFound);
        }

        // get folder_id
        self.cache_discs(album_id).await?;
        let folder_id = self.get_parent_folder(album_id, Some(disc_id));

        // get audio file id
        let permit = self.semaphore.acquire().await.unwrap();
        let (_, list) = self.prepare_list()
            .q(&format!("trashed = false and name contains '{:02}.' and '{}' in parents", track_id, folder_id))
            .param("fields", "nextPageToken, files(id,name,fileExtension,size)")
            .doit().await?;
        drop(permit);

        let files = list.files.unwrap();
        let id = files.iter().reduce(|a, b| if a.name.as_ref().unwrap().starts_with(&format!("{:02}.", track_id)) { a } else { b });
        match id {
            Some(file) => {
                let reader = self.get_file(file.id.as_ref().unwrap()).await?;
                let (info, reader) = crate::utils::read_header(reader).await?;
                Ok(BackendReaderExt {
                    extension: file.file_extension.as_ref().unwrap().to_string(),
                    size: usize::from_str(file.size.as_ref().unwrap()).unwrap(),
                    duration: info.total_samples / info.sample_rate as u64,
                    reader,
                })
            }
            None => Err(BackendError::FileNotFound),
        }
    }

    async fn get_cover(&self, album_id: &str, disc_id: Option<u8>) -> Result<BackendReader, BackendError> {
        // album_id not found
        if !self.folders.contains_key(album_id) ||
            // disc not found
            (disc_id.is_some() && !matches!(disc_id, Some(1)) && !self.discs.contains_key(album_id)) {
            return Err(BackendError::FileNotFound);
        }

        // get folder_id
        self.cache_discs(album_id).await?;
        let folder_id = self.get_parent_folder(album_id, disc_id);

        // get cover file id
        let permit = self.semaphore.acquire().await.unwrap();
        let (_, list) = self.prepare_list()
            .q(&format!("trashed = false and mimeType = 'image/jpeg' and name = 'cover.jpg' and '{}' in parents", folder_id))
            .param("fields", "nextPageToken, files(id,name)")
            .doit().await?;
        drop(permit);

        // get cover file & return
        let files = list.files.unwrap();
        let file = files.get(0).ok_or(BackendError::FileNotFound)?;
        self.get_file(file.id.as_ref().unwrap()).await
    }
}
