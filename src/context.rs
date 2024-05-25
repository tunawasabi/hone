//! This module contains the context of the application.
use crate::config::Config;
use std::sync::OnceLock;

static CONFIG_CONTEXT: OnceLock<Config> = OnceLock::new();
pub struct ConfigContext;
impl ConfigContext {
    pub fn get() -> Option<&'static Config> {
        CONFIG_CONTEXT.get()
    }

    pub fn set(config: Config) -> Result<(), ()> {
        CONFIG_CONTEXT.set(config).map_err(|_| ())
    }
}
