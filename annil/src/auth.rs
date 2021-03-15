use actix_web::HttpRequest;
use jwt_simple::prelude::*;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use crate::share::SharePayload;
use sqlx::{Pool, Postgres};
use crate::db::iat_valid;

pub(crate) trait CanFetch {
    fn can_fetch(&self, catalog: &str, track_id: Option<u8>) -> bool;
}

#[derive(Serialize, Deserialize)]
pub(crate) struct UserClaim {
    #[serde(rename = "type")]
    claim_type: String,
    pub(crate) username: String,
    #[serde(rename = "allowShare")]
    pub(crate) allow_share: bool,
}

impl CanFetch for UserClaim {
    fn can_fetch(&self, _: &str, _: Option<u8>) -> bool {
        true
    }
}

pub(crate) enum AnnilClaims {
    User(UserClaim),
    Share(SharePayload),
}

impl CanFetch for AnnilClaims {
    fn can_fetch(&self, catalog: &str, track_id: Option<u8>) -> bool {
        match &self {
            AnnilClaims::User(u) => u.can_fetch(catalog, track_id),
            AnnilClaims::Share(s) => s.can_fetch(catalog, track_id),
        }
    }
}

fn auth_impl<T: Serialize + DeserializeOwned>(jwt: &str, key: &HS256Key) -> Result<JWTClaims<T>, ()> {
    let claims = key.verify_token::<T>(jwt, None).map_err(|_| ())?;
    Ok(claims)
}

fn auth_user(jwt: &str, key: &HS256Key) -> Option<JWTClaims<UserClaim>> {
    match auth_impl::<UserClaim>(jwt, key) {
        Ok(c) => if c.custom.claim_type == "user" { Some(c) } else { None },
        Err(_) => None,
    }
}

fn auth_share(jwt: &str, key: &HS256Key) -> Option<JWTClaims<SharePayload>> {
    match auth_impl::<SharePayload>(jwt, key) {
        Ok(c) => if c.custom.claim_type == "share" { Some(c) } else { None },
        Err(_) => None,
    }
}

fn auth_header(req: &HttpRequest) -> Option<&str> {
    let header = req.headers()
        .get("Authorization")?
        .to_str().ok()?;
    if header.starts_with("Bearer ") {
        Some(&header[7..])
    } else {
        Some(header)
    }
}

pub(crate) async fn auth_user_or_share(req: &HttpRequest, key: &HS256Key, pool: Pool<Postgres>) -> Option<AnnilClaims> {
    let header = auth_header(req)?;
    if let Some(user) = auth_user(header, key) {
        if user.issued_at.is_none() || !iat_valid(pool, &user.custom.username, user.issued_at.unwrap().as_secs()).await {
            return None;
        }
        return Some(AnnilClaims::User(user.custom));
    }
    let share = auth_share(header, key)?;
    if share.issued_at.is_none() || !iat_valid(pool, &share.custom.username, share.issued_at.unwrap().as_secs()).await {
        return None;
    }
    Some(AnnilClaims::Share(share.custom))
}

pub(crate) async fn auth_user_can_share(req: &HttpRequest, key: &HS256Key, pool: Pool<Postgres>) -> Option<UserClaim> {
    let header = auth_header(req)?;
    let user = auth_user(header, key)?;
    if user.issued_at.is_none() || !iat_valid(pool, &user.custom.username, user.issued_at.unwrap().as_secs()).await {
        return None;
    }
    Some(user.custom)
}

#[test]
fn test_sign() {
    let key = HS256Key::from_bytes(b"a token here");
    let now = Some(Clock::now_since_epoch());
    let jwt = key.authenticate(
        JWTClaims {
            issued_at: now,
            expires_at: None,
            invalid_before: None,
            issuer: None,
            subject: None,
            audiences: None,
            jwt_id: None,
            nonce: None,
            custom: UserClaim {
                claim_type: "user".to_string(),
                username: "test".to_string(),
                allow_share: true,
            },
        }
    ).unwrap();
}