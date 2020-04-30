use config;
use serde_derive::Deserialize;
use std::collections::HashMap;
use svc_agent::{mqtt::AgentConfig, AccountId};

use crate::event::Event;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) id: AccountId,
    pub(crate) agent_label: String,
    pub(crate) broker_id: AccountId,
    pub(crate) mqtt: AgentConfig,
    #[serde(default)]
    pub(crate) events: HashMap<Event, u64>,
}

pub(crate) fn load() -> Result<Config, config::ConfigError> {
    let mut parser = config::Config::default();
    parser.merge(config::File::with_name("App"))?;
    parser.merge(config::Environment::with_prefix("APP").separator("__"))?;
    parser.try_into::<Config>()
}
