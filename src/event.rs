use chrono::Utc;
use serde_derive::{Deserialize, Serialize};
use svc_agent::{
    mqtt::{
        IntoPublishableMessage, OutgoingEvent, OutgoingEventProperties, ShortTermTimingProperties,
    },
    AccountId,
};

use crate::config::Config;

#[derive(Debug, Default, Serialize)]
struct Payload {
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<u64>,
}

impl Payload {
    fn new() -> Self {
        Default::default()
    }

    fn set_duration(&mut self, duration: u64) -> &mut Self {
        self.duration = Some(duration);
        self
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    #[serde(rename = "metric.pull")]
    MetricPull,
    #[serde(rename = "system.vacuum")]
    SystemVacuum,
    #[serde(rename = "room.notify_opened")]
    RoomNotifyOpened,
}

impl Event {
    pub(crate) fn into_message(self, config: &Config) -> Box<dyn IntoPublishableMessage + Send> {
        let mut payload = Payload::new();

        if let Some(duration) = config.events.get(&self) {
            payload.set_duration(*duration);
        }

        match self {
            Self::MetricPull => {
                let props = build_props("metric.pull");
                Box::new(OutgoingEvent::broadcast(payload, props, "events"))
            }
            Self::SystemVacuum => {
                let props = build_props("system.vacuum");
                let to = svc_account(config, "conference");
                Box::new(OutgoingEvent::multicast(payload, props, &to))
            }
            Self::RoomNotifyOpened => {
                let props = build_props("room.notify_opened");
                let to = svc_account(config, "conference");
                Box::new(OutgoingEvent::multicast(payload, props, &to))
            }
        }
    }
}

fn build_props(label: &'static str) -> OutgoingEventProperties {
    let timing = ShortTermTimingProperties::new(Utc::now());
    OutgoingEventProperties::new(label, timing)
}

fn svc_account(config: &Config, label: &'static str) -> AccountId {
    AccountId::new(label, &config.svc_audience)
}
