use crate::{AnnilClaims, AppState};
use actix_web::http::header::{ETAG, IF_NONE_MATCH};
use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use std::borrow::Cow;
use std::collections::HashSet;

/// Get available albums of current annil server
#[get("/albums")]
async fn albums(
    claims: AnnilClaims,
    data: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    match claims {
        AnnilClaims::User(_) => {
            if let Some(Ok(mut etag)) = req.headers().get(IF_NONE_MATCH).map(|v| v.to_str()) {
                if etag.starts_with("W/") {
                    etag = &etag[2..];
                }
                if etag == data.etag.read().as_str() {
                    return HttpResponse::NotModified().finish();
                }
            }

            let mut albums: HashSet<Cow<str>> = HashSet::new();
            let read = data.providers.read();

            // users can get real album list
            for provider in read.iter() {
                albums.extend(provider.albums().await);
            }
            HttpResponse::Ok()
                .append_header((ETAG, format!(r#"W/{}"#, data.etag.read())))
                .json(albums)
        }
        AnnilClaims::Share(share) => {
            // guests can only get album list defined in jwt
            HttpResponse::Ok().json(share.audios.keys().collect::<Vec<_>>())
        }
    }
}
