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
                }, oauth2::InstalledFlowReturnMethod::HTTPRedirect)
                    .persist_tokens_to_disk("/tmp/anni_token")
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
    hub: DriveHub,
    /// HashMap mapping Catalog and folder_id
    folders: HashMap<String, String>,
    /// Settings
    settings: DriveBackendSettings,
}

impl DriveBackend {
    pub async fn new(auth: DriveAuth, settings: DriveBackendSettings) -> Result<Self, BackendError> {
        let auth = auth.build(&settings.token_path).await?;
        auth.token(&[
            "https://www.googleapis.com/auth/drive.metadata.readonly",
            "https://www.googleapis.com/auth/drive.readonly",
        ]).await?;
        let hub = DriveHub::new(hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()), auth);
        Ok(Self {
            hub,
            folders: Default::default(),
            settings,
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

    async fn get_file(&self, file_id: &str) -> Result<BackendReader, BackendError> {
        let (resp, _) = self.hub.files().get(file_id)
            .supports_all_drives(true)
            .param("alt", "media")
            .doit().await?;
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
        let mut page_token = String::new();
        loop {
            let (_, list) = self.prepare_list()
                .page_token(&page_token)
                .q("mimeType = 'application/vnd.google-apps.folder' and trashed = false")
                .param("fields", "nextPageToken, files(id,name)")
                .doit().await?;
            for file in list.files.unwrap() {
                let name = file.name.unwrap();
                match album_info(&name) {
                    Ok((_, catalog, _, disc_count)) => {
                        if disc_count == 1 {
                            self.folders.insert(catalog, file.id.unwrap());
                        }
                    }
                    Err(_) => {
                        match disc_info(&name) {
                            Ok((catalog, ..)) => {
                                self.folders.insert(catalog, file.id.unwrap());
                            }
                            Err(_) => {}
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

    async fn get_audio(&self, catalog: &str, track_id: u8) -> Result<BackendReaderExt, BackendError> {
        // catalog not found
        if !self.folders.contains_key(catalog) {
            return Err(BackendError::UnknownCatalog);
        }
        // get audio file id
        let (_, list) = self.prepare_list()
            .q(&format!("trashed = false and name contains '{:02}.' and '{}' in parents", track_id, self.folders[catalog]))
            .param("fields", "nextPageToken, files(id,name,fileExtension,size)")
            .doit().await?;
        let files = list.files.unwrap();
        // TODO: check whether the following line is correct
        let id = files.iter().reduce(|a, b| if a.name.as_ref().unwrap().starts_with(&format!("{:02}.", track_id)) { a } else { b });
        match id {
            Some(file) => {
                let reader = self.get_file(file.id.as_ref().unwrap()).await?;
                Ok(BackendReaderExt {
                    extension: file.file_extension.as_ref().unwrap().to_string(),
                    size: u64::from_str(file.size.as_ref().unwrap()).unwrap(),
                    reader,
                })
            }
            None => Err(BackendError::FileNotFound),
        }
    }

    async fn get_cover(&self, catalog: &str) -> Result<BackendReader, BackendError> {
        // catalog not found
        if !self.folders.contains_key(catalog) {
            return Err(BackendError::UnknownCatalog);
        }
        // get cover file id
        let (_, list) = self.prepare_list()
            .q(&format!("trashed = false and mimeType = 'image/jpeg' and name = 'cover.jpg' and '{}' in parents", self.folders[catalog]))
            .param("fields", "nextPageToken, files(id,name)")
            .doit().await?;
        let files = list.files.unwrap();
        // TODO: check whether the following line is correct
        let id = files.iter().reduce(|a, b| if let Some("cover.jpg") = a.name.as_deref() { a } else { b });
        match id {
            Some(file) => self.get_file(file.id.as_ref().unwrap()).await,
            None => Err(BackendError::FileNotFound),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::backends::drive::{DriveBackend, DriveBackendSettings};
    use crate::Backend;

    #[tokio::test]
    async fn test_oauth() {
        let mut drive = DriveBackend::new(Default::default(), DriveBackendSettings {
            corpora: "drive".to_string(),
            drive_id: Some("0AJIJiIDxF1yBUk9PVA".to_string()),
            token_path: "/tmp/anni_token".to_string(),
        }).await.unwrap();
        drive.albums().await.unwrap();
        drive.get_cover("FVS-SAKOST-02").await.unwrap();
        drive.get_audio("TGCS-10948", 1).await.unwrap();
    }
}