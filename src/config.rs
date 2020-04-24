use config;
use serde_derive::Deserialize;
use std::collections::HashMap;
use svc_agent::{mqtt::AgentConfig, AccountId};
use svc_authn::jose::Algorithm;

use crate::event::Event;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) id: AccountId,
    pub(crate) id_token: JwtConfig,
    pub(crate) agent_label: String,
    pub(crate) broker_id: AccountId,
    pub(crate) mqtt: AgentConfig,
    #[serde(default = "default_events")]
    pub(crate) events: HashMap<Event, u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct JwtConfig {
    #[serde(deserialize_with = "svc_authn::serde::algorithm")]
    pub(crate) algorithm: Algorithm,
    #[serde(deserialize_with = "svc_authn::serde::file")]
    pub(crate) key: Vec<u8>,
}

pub(crate) fn load() -> Result<Config, config::ConfigError> {
    let mut parser = config::Config::default();
    parser.merge(config::File::with_name("App"))?;
    parser.merge(config::Environment::with_prefix("APP").separator("__"))?;
    parser.try_into::<Config>()
}

fn default_events() -> HashMap<Event, u64> {
    HashMap::new()
}
