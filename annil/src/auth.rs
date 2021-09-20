use actix_web::{HttpRequest, HttpMessage, FromRequest, web};
use jwt_simple::prelude::*;
use serde::{Serialize, Deserialize};

use std::task::{Context, Poll};

use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error};
use actix_utils::future::{ok, Ready};
use actix_web::web::Query;
use actix_web::dev::{Transform, Service, Payload};
use futures::future::Either;
use crate::error::AnnilError;
use crate::AppState;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct UserClaim {
    pub(crate) username: String,
    #[serde(rename = "allowShare")]
    pub(crate) allow_share: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct ShareClaim {
    pub(crate) username: String,
    pub(crate) audios: HashMap<String, Vec<u8>>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum AnnilClaims {
    User(UserClaim),
    Share(ShareClaim),
}

impl FromRequest for AnnilClaims {
    type Config = ();
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
    pub(crate) fn can_fetch(&self, catalog: &str, track_id: Option<u8>) -> bool {
        match &self {
            AnnilClaims::User(_) => true,
            AnnilClaims::Share(s) => s.audios.contains_key(catalog) && track_id.map(|i| s.audios[catalog].contains(&i)).unwrap_or(true),
        }
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
        #[derive(Deserialize)]
        struct AuthQuery {
            auth: String,
        }

        // get Authorization from header / query
        let auth = req.headers()
            .get("Authorization")
            .map_or(
                Query::<AuthQuery>::from_query(req.query_string())
                    .ok()
                    .map(|q| q.into_inner().auth),
                |header| header.to_str().ok().map(|r| r.to_string()),
            );
        match auth {
            Some(auth) => {
                let data = req.app_data::<web::Data<AppState>>().unwrap();
                match data.key.verify_token::<AnnilClaims>(&auth, None) {
                    Ok(token) => {
                        req.extensions_mut().insert(token.custom);
                        Either::Left(self.service.call(req))
                    }
                    Err(e) => {
                        println!("{:?}", e);
                        Either::Right(ok(req.error_response(AnnilError::Unauthorized)))
                    }
                }
            }
            None => Either::Right(ok(req.error_response(AnnilError::Unauthorized)))
        }
    }
}

#[test]
fn test_sign() {
    let key = HS256Key::from_bytes(b"a token here");
    let jwt = key.authenticate(
        JWTClaims {
            issued_at: Some(0.into()),
            expires_at: None,
            invalid_before: None,
            issuer: None,
            subject: None,
            audiences: None,
            jwt_id: None,
            nonce: None,
            custom: UserClaim {
                username: "test".to_string(),
                allow_share: true,
            },
        }
    ).expect("failed to sign jwt");
    assert_eq!(jwt, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpYXQiOjAsInR5cGUiOiJ1c2VyIiwidXNlcm5hbWUiOiJ0ZXN0IiwiYWxsb3dTaGFyZSI6dHJ1ZX0.7CH27OBvUnJhKxBdtZbJSXA-JIwQ4MWqI5JsZ46NoKk");
}