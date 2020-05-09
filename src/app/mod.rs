use std::time::Duration;

use async_std::{prelude::*, task};
use chrono::Utc;
use log::{error, info, warn};
use serde_derive::Serialize;
use serde_json::json;
use svc_authn::token::jws_compact;

use svc_agent::{
    mqtt::{
        compat, Agent, AgentBuilder, ConnectionMode, IntoPublishableDump, Notification,
        OutgoingEvent, OutgoingEventProperties, OutgoingRequest, QoS, ShortTermTimingProperties,
    },
    Addressable, AgentId, Authenticable, SharedGroup, Subscription,
};

use crate::config::Config;
use crate::event::Event;

const API_VERSION: &str = "v1";

#[derive(Debug, Serialize)]
struct SubscriptionRequest {
    subject: AgentId,
    object: Vec<String>,
}

impl SubscriptionRequest {
    fn new(subject: AgentId, object: Vec<&str>) -> Self {
        Self {
            subject,
            object: object.iter().map(|&s| s.into()).collect(),
        }
    }
}

pub(crate) async fn run(config: &Config) -> Result<(), String> {
    let agent_id = AgentId::new(&config.agent_label, config.id.clone());
    info!("Agent id: {:?}", &agent_id);

    let token = jws_compact::TokenBuilder::new()
        .issuer(&agent_id.as_account_id().audience().to_string())
        .subject(&agent_id)
        .key(config.id_token.algorithm, config.id_token.key.as_slice())
        .build()
        .map_err(|err| format!("Error creating an id token: {}", err))?;

    let mut agent_config = config.mqtt.clone();
    agent_config.set_password(&token);

    let (mut agent, rx) = AgentBuilder::new(agent_id.clone(), API_VERSION)
        .connection_mode(ConnectionMode::Service)
        .start(&agent_config)
        .map_err(|err| format!("Failed to create an agent: {}", err))?;

    let (mq_tx, mq_rx) = futures::channel::mpsc::unbounded::<Notification>();

    std::thread::spawn(move || {
        for message in rx {
            if mq_tx.unbounded_send(message).is_err() {
                error!("Error sending message to the internal channel");
            }
        }
    });

    let group = SharedGroup::new("loadbalancer", agent_id.as_account_id().clone());
    agent
        .subscribe(
            &Subscription::multicast_requests(Some(API_VERSION)),
            QoS::AtMostOnce,
            Some(&group),
        )
        .map_err(|err| format!("Error subscribing to multicast requests: {}", err))?;

    spawn_subscriptions_handler(agent.clone(), mq_rx);

    send_events(&mut agent, &config).await?;

    Ok(())
}

async fn send_events(agent: &mut Agent, config: &Config) -> Result<(), String> {
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

fn spawn_subscriptions_handler(
    agent: Agent,
    mut mq_rx: futures::channel::mpsc::UnboundedReceiver<Notification>,
) {
    task::spawn(async move {
        while let Some(message) = mq_rx.next().await {
            let mut agent = agent.clone();
            task::spawn(async move {
                match message {
                    svc_agent::mqtt::Notification::Publish(message) => {
                        let message_bytes = message.payload;
                        info!(
                            "Incoming message = '{}'",
                            String::from_utf8_lossy(&message_bytes)
                        );

                        if let Err(e) = handle_subscription_request(&mut agent, &message_bytes) {
                            error!("Failed to handle subscription request: {}", e);
                        }
                    }
                    _ => {
                        warn!("Unsupported notification type = '{:?}'", message);
                    }
                };
            });
        }
    });
}

fn handle_subscription_request(agent: &mut Agent, message_bytes: &[u8]) -> Result<(), String> {
    let start_timestamp = Utc::now();

    let envelope = serde_json::from_slice::<compat::IncomingEnvelope>(&message_bytes)
        .map_err(|err| format!("Failed to parse incoming envelope: {}", err))?;

    match envelope.properties() {
        compat::IncomingEnvelopeProperties::Request(ref reqp) => {
            match reqp.method() {
                "kruonis.subscribe" => {
                    let payload =
                        SubscriptionRequest::new(reqp.as_agent_id().to_owned(), vec!["events"]);

                    let short_term_timing = ShortTermTimingProperties::until_now(start_timestamp);

                    let props = reqp.to_request(
                        "subscription.create",
                        reqp.response_topic(),
                        reqp.correlation_data(),
                        short_term_timing,
                    );

                    // FIXME: It looks like sending a request to the client but the broker intercepts it
                    //        creates a subscription and replaces the request with the response.
                    //        This is kind of ugly but it guaranties that the request will be processed by
                    //        the broker node where the client is connected to. We need that because
                    //        the request changes local state on that node.
                    //        A better solution will be possible after resolution of this issue:
                    //        https://github.com/vernemq/vernemq/issues/1326.
                    //        Then we won't need the local state on the broker at all and will be able
                    //        to send a multicast request to the broker.
                    let outgoing_request =
                        OutgoingRequest::unicast(payload, props, reqp, API_VERSION);
                    let message = Box::new(outgoing_request) as Box<dyn IntoPublishableDump + Send>;
                    let dump = message
                        .into_dump(agent.address())
                        .map_err(|err| format!("Failed to dump message: {}", err))?;

                    info!(
                        "Outgoing message = '{}' sending to the topic = '{}'",
                        dump.payload(),
                        dump.topic(),
                    );

                    agent
                        .publish_dump(dump)
                        .map_err(|err| format!("Failed to publish message: {}", err))
                }
                method => {
                    warn!("Unexpected request method: {:?}", method);
                    Ok(())
                }
            }
        }
        val => {
            warn!("Unexpected message type: {:?}", val);
            Ok(())
        }
    }
}
