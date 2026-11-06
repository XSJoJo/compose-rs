//! Compose's `tooling` package: what tools see of compositions. Each part is behind a cargo
//! feature of its own, off by default, so a build without tools pays nothing for them.
//!
//! - `composition-observer`: `CompositionObserver`, a composition's passes and scopes as they
//!   happen (D1505).
//! - `composition-data`: `CompositionData`, a composition's slot table read as a tree of groups,
//!   and `LOCAL_INSPECTION_TABLES`, which collects the compositions under a place (D1509).

#[cfg(feature = "composition-data")]
mod data;
#[cfg(feature = "composition-observer")]
mod observer;

#[cfg(feature = "composition-data")]
pub use data::*;
#[cfg(feature = "composition-observer")]
pub use observer::*;
