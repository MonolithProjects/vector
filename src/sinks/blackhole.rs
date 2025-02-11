use crate::{
    buffers::Acker,
    config::{DataType, GenerateConfig, SinkConfig, SinkContext, SinkDescription},
    emit,
    internal_events::BlackholeEventReceived,
    sinks::util::StreamSink,
};
use async_trait::async_trait;
use futures::{future, stream::BoxStream, FutureExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::sleep_until;
use vector_core::event::Event;
use vector_core::ByteSizeOf;

pub struct BlackholeSink {
    total_events: usize,
    total_raw_bytes: usize,
    config: BlackholeConfig,
    acker: Acker,
    last: Option<Instant>,
}

#[derive(Clone, Debug, Derivative, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
#[derivative(Default)]
pub struct BlackholeConfig {
    #[derivative(Default(value = "1000"))]
    #[serde(default = "default_print_amount")]
    pub print_amount: usize,
    pub rate: Option<usize>,
}

fn default_print_amount() -> usize {
    1_000
}

inventory::submit! {
    SinkDescription::new::<BlackholeConfig>("blackhole")
}

impl GenerateConfig for BlackholeConfig {
    fn generate_config() -> toml::Value {
        toml::Value::try_from(&Self::default()).unwrap()
    }
}

#[async_trait::async_trait]
#[typetag::serde(name = "blackhole")]
impl SinkConfig for BlackholeConfig {
    async fn build(
        &self,
        cx: SinkContext,
    ) -> crate::Result<(super::VectorSink, super::Healthcheck)> {
        let sink = BlackholeSink::new(self.clone(), cx.acker());
        let healthcheck = future::ok(()).boxed();

        Ok((super::VectorSink::Stream(Box::new(sink)), healthcheck))
    }

    fn input_type(&self) -> DataType {
        DataType::Any
    }

    fn sink_type(&self) -> &'static str {
        "blackhole"
    }
}

impl BlackholeSink {
    pub fn new(config: BlackholeConfig, acker: Acker) -> Self {
        BlackholeSink {
            config,
            total_events: 0,
            total_raw_bytes: 0,
            acker,
            last: None,
        }
    }
}

#[async_trait]
impl StreamSink for BlackholeSink {
    async fn run(&mut self, input: BoxStream<'_, Event>) -> Result<(), ()> {
        let mut chunks = input.chunks(self.config.print_amount);
        while let Some(events) = chunks.next().await {
            if let Some(rate) = self.config.rate {
                let factor: f32 = 1.0 / rate as f32;
                let secs: f32 = factor * (events.len() as f32);
                let until = self.last.unwrap_or_else(Instant::now) + Duration::from_secs_f32(secs);
                sleep_until(until.into()).await;
                self.last = Some(until);
            }

            let message_len = events.size_of();

            self.total_events += events.len();
            self.total_raw_bytes += message_len;

            emit!(BlackholeEventReceived {
                byte_size: message_len
            });

            info!({
                events = self.total_events,
                raw_bytes_collected = self.total_raw_bytes
            }, "Total events collected");

            self.acker.ack(events.len());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::random_events_with_stream;

    #[test]
    fn generate_config() {
        crate::test_util::test_generate_config::<BlackholeConfig>();
    }

    #[tokio::test]
    async fn blackhole() {
        let config = BlackholeConfig {
            print_amount: 10,
            rate: None,
        };
        let mut sink = BlackholeSink::new(config, Acker::Null);

        let (_input_lines, events) = random_events_with_stream(100, 10, None);
        let _ = sink.run(Box::pin(events)).await.unwrap();
    }
}
