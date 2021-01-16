use rocket::Route;

pub mod api;

pub const PATH: &'static str = "/rest";

pub fn routes() -> Vec<Route> {
    routes![]
}