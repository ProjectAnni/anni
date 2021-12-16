use actix_web::{web, Responder, HttpResponse, post, ResponseError};
use crate::AppState;
use actix_web::http::header::ContentType;
use std::collections::HashMap;
use serde::Deserialize;
use jwt_simple::algorithms::MACLike;
use jwt_simple::prelude::{Duration, Clock, JWTClaims};
use crate::auth::{AnnilClaims, ShareClaim};
use crate::error::AnnilError;

#[derive(Deserialize, Clone)]
pub(crate) struct SharePayload {
    audios: HashMap<String, HashMap<String, Vec<u8>>>,
    #[serde(skip_serializing, default)]
    expire: u64,
}

/// Create share jwt token
#[post("/share")]
pub(crate) async fn share(key: AnnilClaims, info: web::Json<SharePayload>, data: web::Data<AppState>) -> impl Responder {
    match key {
        AnnilClaims::User(user) => {
            if user.allow_share {
                let claims = {
                    let now = Some(Clock::now_since_epoch());
                    let expires_at = if info.expire > 0 {
                        Some(now.unwrap() + Duration::from_hours(info.expire))
                    } else {
                        None
                    };

                    let custom = ShareClaim { username: user.username, audios: info.into_inner().audios };
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
                let jwt = data.key.authenticate::<ShareClaim>(claims).unwrap();
                HttpResponse::Ok()
                    .content_type(ContentType::plaintext())
                    .body(jwt)
            } else {
                AnnilError::NoPermission.error_response()
            }
        }
        _ => AnnilError::Unauthorized.error_response(),
    }
}
