use std::time::Duration;

use async_std::prelude::*;
use chrono::Utc;
use log::info;
use serde_json::json;

use svc_agent::{
    mqtt::{
        AgentBuilder, ConnectionMode, IntoPublishableDump, OutgoingEvent, OutgoingEventProperties,
        ShortTermTimingProperties,
    },
    AgentId
};

use crate::event::Event;

const API_VERSION: &str = "v1";

#[async_std::main]
async fn main() -> Result<(), String> {
    env_logger::init();

    let config = config::load().map_err(|err| format!("Failed to load config: {}", err))?;
    info!("App config: {:?}", config);

    let agent_id = AgentId::new(&config.agent_label, config.id.clone());
    info!("Agent id: {:?}", &agent_id);

    let (mut agent, _rx) = AgentBuilder::new(agent_id.clone(), API_VERSION)
        .connection_mode(ConnectionMode::Service)
        .start(&config.mqtt)
        .map_err(|err| format!("Failed to create an agent: {}", err))?;

    let interval_streams: Vec<_> = config
        .events
        .iter()
        .map(|(event, secs)| {
            async_std::stream::interval(Duration::from_secs(*secs)).map(move |_| *event)
        })
        .collect();

    let mut interval_stream = futures::stream::select_all(interval_streams);

    while let Some(event) = interval_stream.next().await {
        let timing = ShortTermTimingProperties::new(Utc::now());
        let props = match event {
            Event::SystemVacuum => OutgoingEventProperties::new("system.vacuum", timing),
            Event::MetricPull => OutgoingEventProperties::new("metric.pull", timing),
        };
        let event = OutgoingEvent::broadcast(json!({}), props, "events");
        let message = Box::new(event) as Box<dyn IntoPublishableDump + Send>;

        let dump = message
            .into_dump(agent.address())
            .map_err(|err| format!("Failed to dump message: {}", err))?;

        info!(
            "Outgoing message = '{}' sending to the topic = '{}'",
            dump.payload(),
            dump.topic(),
        );

        agent
            .publish_dump(dump.clone())
            .map_err(|err| format!("Failed to publish message: {}", err))?;
    }

    Ok(())
}

mod config;
mod event;
