//! This module contains the context of the application.
use crate::config::Config;
use std::sync::OnceLock;

static CONFIG_CONTEXT: OnceLock<Config> = OnceLock::new();
pub struct ConfigContext;
impl ConfigContext {
    pub fn get() -> &'static Config {
        let Some(ctx) = CONFIG_CONTEXT.get() else {
            panic!("ConfigContext is not initialized. You must call ConfigContext::set before using ConfigContext::get");
        };

        ctx
    }

    pub fn set(config: Config) -> Result<(), ()> {
        CONFIG_CONTEXT.set(config).map_err(|_| ())
    }
}
