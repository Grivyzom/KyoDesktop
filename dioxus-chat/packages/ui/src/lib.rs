//! This crate contains all shared UI for the workspace.

mod hero;
pub use hero::Hero;

mod navbar;
pub use navbar::Navbar;

mod echo;
pub use echo::Echo;

pub mod chat;
pub use chat::*;

pub mod sidebar;
pub use sidebar::*;

pub mod models;
pub use models::*;

pub mod dashboard;
pub use dashboard::*;

pub mod sidecar;
pub use sidecar::*;

pub mod settings;
pub use settings::*;

pub mod vault;
pub use vault::*;

pub mod voice;
pub use voice::*;

pub mod transfer_progress;
pub use transfer_progress::*;

pub mod music;
pub use music::*;

pub mod oauth;
pub use oauth::*;

pub mod kyo_cli;
pub use kyo_cli::*;

pub mod projects_view;
pub use projects_view::*;

