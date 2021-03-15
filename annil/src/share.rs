use actix_web::{HttpRequest, web, Responder, HttpResponse, post};
use crate::{AppState, auth};
use actix_web::http::header::ContentType;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use jwt_simple::algorithms::MACLike;
use jwt_simple::prelude::{Duration, Clock, JWTClaims};

#[derive(Serialize, Deserialize)]
pub(crate) struct SharePayload {
    audios: HashMap<String, Vec<u8>>,
    #[serde(skip_serializing, default)]
    expire: u64,
}

/// Create share jwt token
#[post("/share")]
pub(crate) async fn share(req: HttpRequest, info: web::Json<SharePayload>, data: web::Data<AppState>) -> impl Responder {
    match auth::auth_user(&req, &data.key) {
        Some(c) => {
            if !c.custom.allow_share {
                return HttpResponse::Forbidden().finish();
            }
        }
        None => {
            return HttpResponse::Unauthorized().finish();
        }
    }

    let claims = {
        let now = Some(Clock::now_since_epoch());
        JWTClaims {
            issued_at: now,
            expires_at: if info.expire > 0 { Some(now.unwrap() + Duration::from_hours(info.expire)) } else { None },
            invalid_before: None,
            issuer: None,
            subject: None,
            audiences: None,
            jwt_id: None,
            nonce: None,
            custom: info.into_inner(),
        }
    };
    let jwt = data.key.authenticate::<SharePayload>(claims).unwrap();
    HttpResponse::Ok()
        .content_type(ContentType::plaintext())
        .body(jwt)
}
