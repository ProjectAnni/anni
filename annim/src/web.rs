//! Embedded Annim Web assets and browser security headers.
//!
//! The operator UI is readable without authentication, but every GraphQL
//! request still crosses the server's admin guard. Assets are compiled into
//! the binary so deployments cannot accidentally serve a stale or writable
//! web root.

use axum::{
    body::Body,
    http::{header, HeaderValue, Response, StatusCode},
};

// These paths intentionally stay outside `annim/src`: the browser client is a
// first-class workspace surface while the release binary embeds one exact copy.
const INDEX: &str = include_str!("../../annim-web/index.html");
const STYLES: &str = include_str!("../../annim-web/styles.css");
const APP: &str = include_str!("../../annim-web/app.js");

const NO_STORE: HeaderValue = HeaderValue::from_static("no-store");
const NOSNIFF: HeaderValue = HeaderValue::from_static("nosniff");
const NO_REFERRER: HeaderValue = HeaderValue::from_static("no-referrer");
const DENY: HeaderValue = HeaderValue::from_static("DENY");
const SAME_ORIGIN: HeaderValue = HeaderValue::from_static("same-origin");
const NO_BROWSER_PERMISSIONS: HeaderValue =
    HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=(), usb=()");
const CSP: HeaderValue = HeaderValue::from_static(
    "default-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'; object-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'self'",
);

pub async fn index() -> Response<Body> {
    asset(INDEX, "text/html; charset=utf-8")
}

pub async fn styles() -> Response<Body> {
    asset(STYLES, "text/css; charset=utf-8")
}

pub async fn app() -> Response<Body> {
    asset(APP, "text/javascript; charset=utf-8")
}

fn asset(content: &'static str, content_type: &'static str) -> Response<Body> {
    let mut response = Response::new(Body::from(content));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(header::CACHE_CONTROL, NO_STORE);
    headers.insert("x-content-type-options", NOSNIFF);
    headers.insert("referrer-policy", NO_REFERRER);
    headers.insert("x-frame-options", DENY);
    headers.insert("cross-origin-opener-policy", SAME_ORIGIN);
    headers.insert("cross-origin-resource-policy", SAME_ORIGIN);
    headers.insert("permissions-policy", NO_BROWSER_PERMISSIONS);
    headers.insert("content-security-policy", CSP);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_asset_uses_the_same_fail_closed_headers() {
        for response in [
            asset(INDEX, "text/html; charset=utf-8"),
            asset(STYLES, "text/css; charset=utf-8"),
            asset(APP, "text/javascript; charset=utf-8"),
        ] {
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL),
                Some(&NO_STORE)
            );
            assert_eq!(
                response.headers().get("x-content-type-options"),
                Some(&NOSNIFF)
            );
            assert_eq!(
                response.headers().get("content-security-policy"),
                Some(&CSP)
            );
            assert_eq!(
                response.headers().get("cross-origin-resource-policy"),
                Some(&SAME_ORIGIN)
            );
        }
    }
}
