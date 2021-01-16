use rocket::response::{content, status};
use rocket::http::Status;

mod system;
pub mod response;
mod error;
mod browsing;

pub type Response = content::Xml<String>;
pub type ResponseNoLongerSupported = status::Custom<&'static str>;

#[inline]
fn no_longer_supported() -> status::Custom<&'static str> {
    status::Custom(Status::Gone, "No longer supported")
}