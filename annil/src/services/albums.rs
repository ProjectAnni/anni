use std::borrow::Cow;
use std::collections::HashSet;
use actix_web::{HttpResponse, Responder, web, get};
use crate::{AnnilClaims, AppState};

/// Get available albums of current annil server
#[get("/albums")]
async fn albums(claims: AnnilClaims, data: web::Data<AppState>) -> impl Responder {
    match claims {
        AnnilClaims::User(_) => {
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