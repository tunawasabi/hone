//! This module contains the context of the application.
use crate::config::Config;
use std::{process::exit, sync::LazyLock};

static CONFIG_CONTEXT: LazyLock<Config> = LazyLock::new(|| {
    Config::read_from("config.toml").unwrap_or_else(|err| {
        println!("{}", err);
        exit(-1);
    })
});

pub struct ConfigContext;
impl ConfigContext {
    pub fn get() -> &'static Config {
        &CONFIG_CONTEXT
    }
}
