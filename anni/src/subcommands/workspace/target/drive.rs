// use crate::workspace::target::WorkspaceTarget;
// use anni_provider::providers::drive::DriveProviderSettings;
// use google_drive3::api::File;
// use google_drive3::hyper::client::HttpConnector;
// use google_drive3::hyper_rustls::HttpsConnector;
// use google_drive3::DriveHub;
// use std::path::Path;

// pub struct WorkspaceDriveTarget {
//     hub: Box<DriveHub<HttpsConnector<HttpConnector>>>,
//     settings: DriveProviderSettings,
// }

// impl WorkspaceTarget for WorkspaceDriveTarget {
//     async fn mkdir<P>(&self, path: P) -> std::io::Result<()>
//     where
//         P: AsRef<Path>,
//     {
//         todo!()
//     }

//     async fn copy<P>(&self, src: P, dst: P) -> std::io::Result<()>
//     where
//         P: AsRef<Path>,
//     {
//         todo!()
//     }
// }
