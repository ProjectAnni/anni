mod admin_reload;
mod admin_sign;
mod albums;
mod audio;
mod cover;
mod info;

pub use albums::*;
pub use audio::*;
pub use cover::*;
pub use info::*;

pub mod admin {
    pub use super::admin_reload::*;
    pub use super::admin_sign::*;
}
