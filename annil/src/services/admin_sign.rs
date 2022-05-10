use actix_web::{HttpResponse, Responder, web, post};
use jwt_simple::prelude::*;
use crate::AppState;
use crate::auth::{UserClaim, UserShare};

#[derive(Deserialize, Clone)]
pub(crate) struct SignPayload {
    user_id: String,
    #[serde(default)]
    share: bool,
}

#[post("/admin/sign")]
async fn sign(data: web::Data<AppState>, info: web::Json<SignPayload>) -> impl Responder {
    let info = info.into_inner();
    let custom = UserClaim {
        user_id: info.user_id,
        share: if info.share {
            Some(UserShare {
                key_id: data.share_key.key_id().as_deref().unwrap().to_string(),
                secret: unsafe { String::from_utf8_unchecked(data.share_key.to_bytes().to_vec()) },
            })
        } else { None },
    };

    let now = Some(Clock::now_since_epoch());
    let claim = JWTClaims {
        issued_at: now,
        expires_at: None,
        invalid_before: None,
        issuer: None,
        subject: None,
        audiences: None,
        jwt_id: None,
        nonce: None,
        custom,
    };
    let token = data.key.authenticate(claim).expect("Failed to sign user token");
    HttpResponse::Ok().body(token)
}
