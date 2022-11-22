use crate::AnnilProvider;

pub async fn compute_etag(providers: &[AnnilProvider]) -> String {
    let mut etag = 0;
    for provider in providers {
        for album in provider.albums().await {
            if let Ok(uuid) = uuid::Uuid::parse_str(album.as_ref()) {
                etag ^= uuid.as_u128();
            } else {
                log::error!("Failed to parse uuid: {album}");
            }
        }
    }
    format!(r#""{}""#, base64::encode(etag.to_be_bytes()))
}
