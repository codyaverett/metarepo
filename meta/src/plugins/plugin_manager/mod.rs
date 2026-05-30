pub mod install;
pub mod lockfile;
pub mod plugin;
pub mod spec;
pub mod verify;

// Export the main plugin
pub use plugin::PluginManagerPlugin;
