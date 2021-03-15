use actix_web::HttpRequest;
use jwt_simple::prelude::*;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use crate::share::SharePayload;

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

fn auth_impl<T: Serialize + DeserializeOwned>(jwt: &str, key: &HS256Key) -> Result<T, ()> {
    let claims = key.verify_token::<T>(jwt, None).map_err(|_| ())?;
    Ok(claims.custom)
}

fn auth_user(jwt: &str, key: &HS256Key) -> Option<UserClaim> {
    match auth_impl::<UserClaim>(jwt, key) {
        Ok(c) => if c.claim_type == "user" { Some(c) } else { None },
        Err(_) => None,
    }
}

fn auth_share(jwt: &str, key: &HS256Key) -> Option<SharePayload> {
    match auth_impl::<SharePayload>(jwt, key) {
        Ok(c) => if c.claim_type == "share" { Some(c) } else { None },
        Err(_) => None,
    }
}

pub(crate) fn auth_header(req: &HttpRequest) -> Option<&str> {
    let header = req.headers()
        .get("Authorization")?
        .to_str().ok()?;
    if header.starts_with("Bearer ") {
        Some(&header[7..])
    } else {
        Some(header)
    }
}

pub(crate) fn auth_user_or_share(req: &HttpRequest, key: &HS256Key) -> Option<AnnilClaims> {
    let header = auth_header(req)?;
    if let Some(user) = auth_user(header, key) {
        return Some(AnnilClaims::User(user));
    }
    let share = auth_share(header, key)?;
    Some(AnnilClaims::Share(share))
}

pub(crate) fn auth_user_can_share(req: &HttpRequest, key: &HS256Key) -> Option<UserClaim> {
    let header = auth_header(req)?;
    auth_user(header, key)
}

#[test]
fn test_sign() {
    let key = HS256Key::from_bytes(b"a token here");
    let jwt = key.authenticate(
        JWTClaims {
            issued_at: None,
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
                allow_share: false,
            },
        }
    ).unwrap();
    assert_eq!(jwt, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ0eXBlIjoidXNlciIsInVzZXJuYW1lIjoidGVzdCIsImFsbG93U2hhcmUiOmZhbHNlfQ.35TW23ypqpICtZ_N_cSM71jp_ckUuGX5vdqAykeVhx8");
}