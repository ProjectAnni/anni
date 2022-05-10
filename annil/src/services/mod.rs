mod cover;
mod info;
mod albums;
mod audio;
mod admin_reload;
mod admin_sign;

pub use cover::*;
pub use info::*;
pub use albums::*;
pub use audio::*;

pub mod admin {
    pub use super::admin_reload::*;
    pub use super::admin_sign::*;
}