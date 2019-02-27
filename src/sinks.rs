use futures::{Async, AsyncSink, Future, Sink};

pub mod cloudwatch;
pub mod console;
pub mod elasticsearch;
pub mod http;
pub mod kafka;
pub mod kinesis;
pub mod prometheus;
pub mod s3;
pub mod splunk;
pub mod tcp;
pub mod util;

use crate::record::Record;

pub type RouterSink = Box<dyn Sink<SinkItem = Record, SinkError = ()> + 'static + Send>;

pub type Healthcheck = Box<dyn Future<Item = (), Error = String> + Send>;

pub struct BlackHole;

impl Sink for BlackHole {
    type SinkItem = Record;
    type SinkError = ();

    fn start_send(
        &mut self,
        _item: Self::SinkItem,
    ) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        Ok(Async::Ready(()))
    }
}
