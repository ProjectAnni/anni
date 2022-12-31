use crate::extractor::token::{AnnilClaim, ShareToken, UserClaim};
use crate::state::AnnilKeys;
use axum::{Extension, Json};
use jwt_simple::prelude::*;
use std::sync::Arc;

#[derive(Deserialize, Clone)]
pub struct SignPayload {
    user_id: String,
    #[serde(default)]
    share: bool,
}

pub async fn sign(
    Extension(keys): Extension<Arc<AnnilKeys>>,
    Json(info): Json<SignPayload>,
) -> String {
    let custom = AnnilClaim::User(UserClaim {
        user_id: info.user_id,
        share: if info.share {
            Some(ShareToken {
                key_id: keys.share_key.key_id().as_deref().unwrap().to_string(),
                secret: unsafe { String::from_utf8_unchecked(keys.share_key.to_bytes().to_vec()) },
                allowed: None,
            })
        } else {
            None
        },
    });

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
    keys.sign_key
        .authenticate(claim)
        .expect("Failed to sign user token")
}
