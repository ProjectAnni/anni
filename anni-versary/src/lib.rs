//! Simple and powerful music backend
//!
//! Provides anni-api and [`subsonic-api`] for clients.
//!
//! [`subsonic-api`]: http://www.subsonic.org/pages/api.jsp
//!
//! ## Usage
//! To launch an anni-versary server, just call its `launch` method:
//! ```rust
//! async fn main() {
//!   anni_versary::launch().await;
//! }
//! ```

#[macro_use]
extern crate rocket;

pub mod subsonic;

use rocket::error::Error;

#[get("/")]
fn hello() -> &'static str {
    "Hello, world!"
}

pub async fn launch() -> Result<(), Error> {
    rocket::ignite()
        .mount("/", routes![hello])
        .mount(subsonic::PATH, subsonic::routes())
        .launch().await
}