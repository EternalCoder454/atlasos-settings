//! What Settings has and how it is reached, without Qt: the pages and the
//! settings on them ([`pages`]), the KCM names that open them ([`kcm`]),
//! search over both ([`search`]) and the launch arguments ([`launch`]).
//!
//! Everything here is static data and pure functions, so the app, the
//! `systemsettings` shim and the KRunner runner answer the same way.

pub mod kcm;
pub mod launch;
pub mod pages;
pub mod search;

pub use pages::{Item, Kind, PAGES, Page};
