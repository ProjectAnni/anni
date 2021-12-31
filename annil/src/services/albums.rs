use std::collections::HashSet;
use actix_web::{HttpResponse, Responder, web, get};
use crate::{AnnilClaims, AppState};

/// Get available albums of current annil server
#[get("/albums")]
async fn albums(claims: AnnilClaims, data: web::Data<AppState>) -> impl Responder {
    match claims {
        AnnilClaims::User(_) => {
            let mut albums: HashSet<&str> = HashSet::new();
            let read = data.backends.read().await;

            // users can get real album list
            for backend in read.iter() {
                albums.extend(backend.albums().into_iter());
            }
            HttpResponse::Ok().json(albums)
        }
        AnnilClaims::Share(share) => {
            // guests can only get album list defined in jwt
            HttpResponse::Ok().json(share.audios.keys().collect::<Vec<_>>())
        }
    }
}