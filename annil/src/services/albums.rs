use std::borrow::Cow;
use std::collections::HashSet;
use actix_web::{HttpResponse, Responder, web, get, HttpRequest};
use actix_web::http::header::IF_NONE_MATCH;
use crate::{AnnilClaims, AppState};

/// Get available albums of current annil server
#[get("/albums")]
async fn albums(claims: AnnilClaims, data: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    match claims {
        AnnilClaims::User(_) => {
            if let Some(Ok(etag)) = req.headers().get(IF_NONE_MATCH).map(|v| v.to_str()) {
                if let Ok(etag) = etag.parse::<u128>() {
                    if etag == *data.etag.read() {
                        return HttpResponse::NotModified().finish();
                    }
                }
            }

            let mut albums: HashSet<Cow<str>> = HashSet::new();
            let read = data.providers.read();

            // users can get real album list
            for provider in read.iter() {
                albums.extend(provider.albums().await);
            }
            HttpResponse::Ok().json(albums)
        }
        AnnilClaims::Share(share) => {
            // guests can only get album list defined in jwt
            HttpResponse::Ok().json(share.audios.keys().collect::<Vec<_>>())
        }
    }
}