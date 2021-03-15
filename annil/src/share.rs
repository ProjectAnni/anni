use actix_web::{HttpRequest, web, Responder, HttpResponse, post};
use crate::{AppState, auth};
use actix_web::http::header::ContentType;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use jwt_simple::algorithms::MACLike;
use jwt_simple::prelude::{Duration, Clock, JWTClaims};
use crate::auth::CanFetch;

#[derive(Serialize, Deserialize)]
pub(crate) struct SharePayload {
    #[serde(skip_deserializing, rename = "type", default = "share_default_type")]
    pub(crate) claim_type: String,
    #[serde(default)]
    pub(crate) username: String,
    pub(crate) audios: HashMap<String, Vec<u8>>,
    #[serde(skip_serializing, default)]
    expire: u64,
}

impl CanFetch for SharePayload {
    fn can_fetch(&self, catalog: &str, track_id: Option<u8>) -> bool {
        self.audios.contains_key(catalog) && track_id.map(|i| self.audios[catalog].contains(&i)).unwrap_or(true)
    }
}

fn share_default_type() -> String {
    "share".to_string()
}

/// Create share jwt token
#[post("/share")]
pub(crate) async fn share(req: HttpRequest, info: web::Json<SharePayload>, data: web::Data<AppState>) -> impl Responder {
    let username = match auth::auth_user_can_share(&req, &data.key, data.pool.clone()).await {
        Some(r) => if r.allow_share {
            r.username
        } else {
            return HttpResponse::Forbidden().finish();
        },
        None => return HttpResponse::Unauthorized().finish(),
    };

    let claims = {
        let now = Some(Clock::now_since_epoch());
        let expires_at = if info.expire > 0 {
            Some(now.unwrap() + Duration::from_hours(info.expire))
        } else {
            None
        };

        let mut custom = info.into_inner();
        custom.username = username;
        JWTClaims {
            issued_at: now,
            expires_at,
            invalid_before: None,
            issuer: None,
            subject: None,
            audiences: None,
            jwt_id: None,
            nonce: None,
            custom,
        }
    };
    let jwt = data.key.authenticate::<SharePayload>(claims).unwrap();
    HttpResponse::Ok()
        .content_type(ContentType::plaintext())
        .body(jwt)
}
