pub mod system;
pub mod response;
pub mod browsing;

mod error;

use rocket::response::{content, status};
use rocket::http::Status;

pub type Response = content::Xml<String>;
pub type ResponseNotImplemented = status::Custom<&'static str>;

#[inline]
fn api_not_implemented() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "Not yet implemented")
}