//! Meta modules: a repo bundling a plugin and/or skills as one discoverable,
//! enable-able unit. See `docs/MODULES.md`.

pub mod discover;
pub mod enable;
mod plugin;
pub mod scan;

pub use plugin::{offer_enable_after_add, ModulePlugin};
