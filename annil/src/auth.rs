use actix_web::HttpRequest;
use jwt_simple::prelude::*;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

#[derive(Serialize, Deserialize)]
pub(crate) struct UserClaim {
    #[serde(rename = "type")]
    claim_type: String,
    user: String,
    #[serde(rename = "allowShare")]
    pub(crate) allow_share: bool,
}

fn auth_impl<T: Serialize + DeserializeOwned>(req: &HttpRequest, key: &HS256Key) -> Result<JWTClaims<T>, ()> {
    let header = req.headers()
        .get("Authorization")
        .ok_or(())?
        .to_str().map_err(|_| ())?;
    if !header.starts_with("Bearer ") {
        return Err(());
    }
    let claims = key.verify_token::<T>(&header[7..], None).map_err(|_| ())?;
    Ok(claims)
}

pub(crate) fn auth_user(req: &HttpRequest, key: &HS256Key) -> Option<JWTClaims<UserClaim>> {
    match auth_impl::<UserClaim>(req, key) {
        Ok(c) => if c.custom.claim_type == "user" { Some(c) } else { None },
        Err(_) => None,
    }
}
