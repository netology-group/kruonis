use serde_derive::Deserialize;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    #[serde(rename = "system.vacuum")]
    SystemVacuum,
    #[serde(rename = "metric.pull")]
    MetricPull
}
