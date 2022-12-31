use crate::error::AnnilError;
use crate::extractor::auth::AuthExtractor;
use crate::extractor::track::TrackIdentifier;
use async_trait::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use jwt_simple::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroU8;
use std::sync::Arc;
use uuid::Uuid;

/// Claim part of Annil token
#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AnnilClaim {
    /// User Token
    User(UserClaim),
    /// Share Token
    Share(ShareClaim),
}

/// `User Token` body
#[derive(Serialize, Deserialize, Clone)]
pub struct UserClaim {
    /// A string indicating the user
    pub(crate) user_id: String,
    /// Optional `share` field, contains properties required to create [Share Token]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) share: Option<ShareToken>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ShareToken {
    /// Key id
    pub(crate) key_id: String,
    /// Secret of corresponding key
    pub(crate) secret: String,
    /// Allowed albums
    pub(crate) allowed: Option<Vec<Uuid>>,
}

/// `Share Token` body
#[derive(Serialize, Deserialize, Clone)]
pub struct ShareClaim {
    pub(crate) audios: HashMap<String, HashMap<String, Vec<NonZeroU8>>>,
}

/// Readonly keys needed
pub struct Keys {
    pub sign_key: HS256Key,
    pub share_key: HS256Key,
    pub admin_token: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for AnnilClaim
where
    Arc<Keys>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AnnilError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthExtractor(auth) = AuthExtractor::from_request_parts(parts, state).await?;
        let keys = Arc::<Keys>::from_ref(state);

        let metadata = Token::decode_metadata(&auth).map_err(|_| AnnilError::Unauthorized)?;
        match metadata.key_id() {
            None => {
                // no key_id, verify with normal token
                if let Ok(token) = keys.sign_key.verify_token::<AnnilClaim>(&auth, None) {
                    // if the token is signed with sign_key, it's always valid
                    return Ok(token.custom);
                }
            }
            Some(_) => {
                // got key_id, verify with share token
                if let Ok(token) = keys.share_key.verify_token::<AnnilClaim>(
                    &auth,
                    Some(VerificationOptions {
                        required_key_id: Some(
                            keys.share_key.key_id().as_deref().unwrap().to_string(),
                        ),
                        ..Default::default()
                    }),
                ) {
                    // We MUST check whether it's a share token here
                    // otherwise, we may get a user token signed by share key
                    if token.custom.is_guest() {
                        return Ok(token.custom);
                    }
                }
            }
        }

        Err(AnnilError::Unauthorized)
    }
}

impl AnnilClaim {
    pub(crate) fn can_fetch(&self, track: &TrackIdentifier) -> bool {
        match &self {
            AnnilClaim::User(_) => true,
            AnnilClaim::Share(s) => {
                match s.audios.get(&track.album_id.to_string()) {
                    // album_id exist
                    Some(album) => match album.get(&format!("{}", track.disc_id)) {
                        // disc_id exist
                        Some(disc) => {
                            // return whether track_id exist in list
                            disc.contains(&track.track_id)
                        }
                        // disc_id does not exist
                        None => false,
                    },
                    // album id does not exist
                    None => false,
                }
            }
        }
    }

    #[inline]
    pub(crate) fn is_guest(&self) -> bool {
        matches!(self, AnnilClaim::Share(_))
    }
}

#[test]
fn test_sign() {
    let key = HS256Key::from_bytes(b"a token here");
    let jwt = key
        .authenticate(JWTClaims {
            issued_at: Some(0.into()),
            expires_at: None,
            invalid_before: None,
            issuer: None,
            subject: None,
            audiences: None,
            jwt_id: None,
            nonce: None,
            custom: AnnilClaim::User(UserClaim {
                user_id: "test".to_string(),
                share: None,
            }),
        })
        .expect("failed to sign jwt");
    assert_eq!(jwt, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpYXQiOjAsInR5cGUiOiJ1c2VyIiwidXNlcl9pZCI6InRlc3QifQ.qBXwC9ILW5GEdTUIt6igJTwwLsuCFCi5sAAvruXQuVM");
}
