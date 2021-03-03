use async_std::{prelude::*, stream};
use chrono::Utc;
use log::{info, warn};
use serde_derive::{Deserialize, Serialize};
use serde_json::json;

use svc_agent::{
    mqtt::{
        Agent, IncomingMessage, IncomingRequestProperties, IntoPublishableMessage, OutgoingRequest,
        OutgoingResponse, ShortTermTimingProperties, SubscriptionTopic,
    },
    AccountId, Addressable, AgentId, Subscription,
};

use super::API_VERSION;

#[derive(Clone, Debug, Serialize)]
struct SubscriptionRequest {
    subject: AgentId,
    object: Vec<String>,
}

impl SubscriptionRequest {
    fn new(subject: AgentId, object: Vec<String>) -> Self {
        Self { subject, object }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CorrelationDataPayload {
    reqp: IncomingRequestProperties,
    subject: AgentId,
    object: Vec<String>,
}

impl CorrelationDataPayload {
    pub fn new(reqp: IncomingRequestProperties, subject: AgentId, object: Vec<String>) -> Self {
        Self {
            reqp,
            subject,
            object,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum CorrelationData {
    SubscriptionCreate(CorrelationDataPayload),
}

impl CorrelationData {
    pub fn dump(&self) -> Result<String, String> {
        serde_json::to_string(self)
            .map_err(|e| format!("Failed to dump correlation data, reason = {:?}", e))
    }

    fn parse(raw_corr_data: &str) -> Result<Self, String> {
        serde_json::from_str::<Self>(raw_corr_data)
            .map_err(|e| format!("Failed to parse correlation data, reason = {:?}", e))
    }
}

type MessageStream = Box<dyn Stream<Item = Box<dyn IntoPublishableMessage + Send>> + Send + Unpin>;

pub async fn handle_subscription_request<T: std::fmt::Debug>(
    agent: &mut Agent,
    message: IncomingMessage<T>,
    self_agent_id: AgentId,
    broker_id: AccountId,
) -> Result<(), String> {
    let start_timestamp = Utc::now();

    let outgoing_message_stream: Result<MessageStream, String> = match message {
        IncomingMessage::Request(req) => {
            let reqp = req.properties();

            match reqp.method() {
                "kruonis.subscribe" => {
                    let subject = reqp.as_agent_id().to_owned();
                    let object = vec!["events".into()];
                    let payload = SubscriptionRequest::new(subject.clone(), object.clone());
                    let broker_id = AgentId::new("nevermind", broker_id);

                    let response_topic = Subscription::unicast_responses_from(&broker_id)
                        .subscription_topic(&self_agent_id, API_VERSION)
                        .map_err(|e| format!("Failed to build response topic, reason = {:?}", e))?
                        .to_owned();

                    let corr_data_payload =
                        CorrelationDataPayload::new(reqp.to_owned(), subject, object);

                    let corr_data = CorrelationData::SubscriptionCreate(corr_data_payload)
                        .dump()
                        .map_err(|e| {
                            format!("Failed to dump correlation data, reason = {:?}", e)
                        })?;

                    let timing = ShortTermTimingProperties::until_now(start_timestamp);

                    let props =
                        reqp.to_request("subscription.create", &response_topic, &corr_data, timing);
                    let outgoing_request =
                        OutgoingRequest::multicast(payload, props, &broker_id, "v1");
                    let boxed_request =
                        Box::new(outgoing_request) as Box<dyn IntoPublishableMessage + Send>;

                    Ok(Box::new(stream::once(boxed_request)) as MessageStream)
                }
                method => {
                    warn!("Unexpected request method: {:?}", method);
                    Ok(Box::new(stream::empty()) as MessageStream)
                }
            }
        }
        IncomingMessage::Response(resp) => {
            match CorrelationData::parse(resp.properties().correlation_data()) {
                Ok(CorrelationData::SubscriptionCreate(corr_data)) => {
                    let timing = ShortTermTimingProperties::until_now(start_timestamp);

                    let props = corr_data
                        .reqp
                        .to_response(svc_agent::mqtt::ResponseStatus::OK, timing);
                    let response = Box::new(OutgoingResponse::unicast(
                        json!({}),
                        props,
                        &corr_data.reqp,
                        API_VERSION,
                    )) as Box<dyn IntoPublishableMessage + Send>;

                    Ok(Box::new(stream::once(response)) as MessageStream)
                }
                Err(err) => {
                    warn!(
                        "Failed to parse response correlation data '{}': {}",
                        resp.properties().correlation_data(),
                        err
                    );

                    Ok(Box::new(stream::empty()) as MessageStream)
                }
            }
        }
        val => {
            warn!("Unexpected message type: {:?}", val);
            Ok(Box::new(stream::empty()) as MessageStream)
        }
    };

    let mut outgoing_message_stream: MessageStream = outgoing_message_stream?;
    while let Some(message) = outgoing_message_stream.next().await {
        let message = message
            .into_dump(agent.address())
            .map_err(|err| format!("Failed to dump message: {}", err))?;

        info!(
            "Outgoing message = '{:?}' sending to the topic = '{}'",
            message.payload(),
            message.topic(),
        );

        agent
            .publish_dump(message)
            .map_err(|err| format!("Failed to publish message, reason = {:?}", err))?;
    }
    Ok(())
}
