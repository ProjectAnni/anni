use actix_web::{web, FromRequest, HttpMessage, HttpRequest};
use jwt_simple::prelude::*;
use serde::{Deserialize, Serialize};

use std::task::{Context, Poll};

use crate::error::AnnilError;
use crate::AppState;
use actix_utils::future::{ok, Ready};
use actix_web::dev::{Payload, Service, Transform};
use actix_web::http::Method;
use actix_web::web::Query;
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error};
use futures::future::Either;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct UserClaim {
    pub(crate) user_id: String,
    pub(crate) share: Option<UserShare>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserShare {
    pub(crate) key_id: String,
    pub(crate) secret: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ShareClaim {
    pub(crate) key_id: String,
    pub(crate) audios: HashMap<String, HashMap<String, Vec<u8>>>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AnnilClaims {
    User(UserClaim),
    Share(ShareClaim),
}

impl FromRequest for AnnilClaims {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        match req.extensions().get::<AnnilClaims>() {
            Some(claim) => ok(claim.clone()),
            None => unreachable!(),
        }
    }
}

impl AnnilClaims {
    pub(crate) fn can_fetch(&self, album_id: &str, disc_id: u8, track_id: u8) -> bool {
        match &self {
            AnnilClaims::User(_) => true,
            AnnilClaims::Share(s) => {
                match s.audios.get(album_id) {
                    // album_id exist
                    Some(album) => match album.get(&format!("{}", disc_id)) {
                        // disc_id exist
                        Some(disc) => {
                            // return whether track_id exist in list
                            disc.contains(&track_id)
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
        matches!(self, AnnilClaims::Share(_))
    }
}

pub struct AnnilAuth;

impl<S> Transform<S, ServiceRequest> for AnnilAuth
    where
        S: Service<ServiceRequest, Response=ServiceResponse, Error=Error>,
        S::Future: 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Transform = AnnilAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AnnilAuthMiddleware { service })
    }
}

pub struct AnnilAuthMiddleware<S> {
    service: S,
}

impl<S> Service<ServiceRequest> for AnnilAuthMiddleware<S>
    where
        S: Service<ServiceRequest, Response=ServiceResponse, Error=Error>,
        S::Future: 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future = Either<S::Future, Ready<Result<Self::Response, Self::Error>>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if matches!(req.method(), &Method::OPTIONS)
            || req.path() == "/info"
            || req.path().ends_with("/cover")
        {
            return Either::Left(self.service.call(req));
        }

        #[derive(Deserialize)]
        struct AuthQuery {
            auth: String,
        }

        // get Authorization from header / query
        let auth = req.headers().get("Authorization").map_or(
            Query::<AuthQuery>::from_query(req.query_string())
                .ok()
                .map(|q| q.into_inner().auth),
            |header| header.to_str().ok().map(|r| r.to_string()),
        );
        match auth {
            Some(auth) => {
                // load app data
                let data = req.app_data::<web::Data<AppState>>().unwrap();

                // for /admin interfaces, treat auth as reload token
                if req.path().starts_with("/admin") {
                    return if auth == data.admin_token {
                        Either::Left(self.service.call(req))
                    } else {
                        Either::Right(ok(req.error_response(AnnilError::Unauthorized)))
                    };
                }

                // for other requests, treat auth as JWT
                // FIXME: handle the unwrap
                let metadata = jwt_simple::token::Token::decode_metadata(&auth).unwrap();
                match metadata.key_id() {
                    None => {
                        // no key_id, verify with normal token
                        if let Ok(token) = data.key.verify_token::<AnnilClaims>(&auth, None) {
                            // notice that share tokens signed by user key are also allowed here
                            // it's somewhat undefined and would never be written to standard, just for convenience
                            req.extensions_mut().insert(token.custom);
                            return Either::Left(self.service.call(req));
                        }
                    }
                    Some(_) => {
                        // got key_id, verify with share token
                        if let Ok(token) = data.share_key.verify_token::<AnnilClaims>(
                            &auth,
                            Some(VerificationOptions {
                                required_key_id: Some(data.share_key.key_id().as_deref().unwrap().to_string()),
                                ..Default::default()
                            }),
                        ) {
                            // note that we MUST check whether it's a share token here
                            // otherwise, we may get a user token signed by share key
                            if token.custom.is_guest() {
                                req.extensions_mut().insert(token.custom);
                                return Either::Left(self.service.call(req));
                            }
                        }
                    }
                }
                Either::Right(ok(req.error_response(AnnilError::Unauthorized)))
            }
            None => Either::Right(ok(req.error_response(AnnilError::Unauthorized))),
        }
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
            custom: AnnilClaims::User(UserClaim {
                user_id: "test".to_string(),
                share: None,
            }),
        })
        .expect("failed to sign jwt");
    assert_eq!(jwt, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpYXQiOjAsInR5cGUiOiJ1c2VyIiwidXNlcl9pZCI6InRlc3QiLCJzaGFyZSI6bnVsbH0.krwh8gkycIVuzPbZ-xZYbRXXzpHD3Lou9OLazsHnmBY");
}
