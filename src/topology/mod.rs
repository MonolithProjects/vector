//! Topology contains all topology based types.
//!
//! Topology is broken up into two main sections. The first
//! section contains all the main topology types include `Topology`
//! and the ability to start, stop and reload a config. The second
//! part contains config related items including config traits for
//! each type of component.

pub mod builder;
pub mod fanout;
mod running;
mod task;

#[cfg(test)]
mod test;

use crate::{
    buffers::{self, EventStream},
    config::{Config, ConfigDiff},
    event::Event,
    topology::{
        builder::Pieces,
        task::{Task, TaskOutput},
    },
};
use futures::{Future, FutureExt};
pub use running::RunningTopology;
use std::{
    collections::HashMap,
    panic::AssertUnwindSafe,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, watch};

type TaskHandle = tokio::task::JoinHandle<Result<TaskOutput, ()>>;

type BuiltBuffer = (
    buffers::BufferInputCloner<Event>,
    Arc<Mutex<Option<Pin<EventStream>>>>,
    buffers::Acker,
);

type Outputs = HashMap<String, fanout::ControlChannel>;

// Watcher types for topology changes. These are currently specific to receiving
// `Outputs`. This could be expanded in the future to send an enum of types if,
// for example, this included a new 'Inputs' type.
type WatchTx = watch::Sender<Outputs>;
pub type WatchRx = watch::Receiver<Outputs>;

pub async fn start_validated(
    config: Config,
    diff: ConfigDiff,
    mut pieces: Pieces,
) -> Option<(RunningTopology, mpsc::UnboundedReceiver<()>)> {
    let (abort_tx, abort_rx) = mpsc::unbounded_channel();

    let mut running_topology = RunningTopology::new(config, abort_tx);

    if !running_topology
        .run_healthchecks(&diff, &mut pieces, running_topology.config.healthchecks)
        .await
    {
        return None;
    }
    running_topology.connect_diff(&diff, &mut pieces).await;
    running_topology.spawn_diff(&diff, pieces);

    Some((running_topology, abort_rx))
}

pub async fn build_or_log_errors(
    config: &Config,
    diff: &ConfigDiff,
    buffers: HashMap<String, BuiltBuffer>,
) -> Option<Pieces> {
    match builder::build_pieces(config, diff, buffers).await {
        Err(errors) => {
            for error in errors {
                error!(message = "Configuration error.", %error);
            }
            None
        }
        Ok(new_pieces) => Some(new_pieces),
    }
}

pub fn take_healthchecks(diff: &ConfigDiff, pieces: &mut Pieces) -> Vec<(String, Task)> {
    (&diff.sinks.to_change | &diff.sinks.to_add)
        .into_iter()
        .filter_map(|name| {
            pieces
                .healthchecks
                .remove(&name)
                .map(move |task| (name, task))
        })
        .collect()
}

async fn handle_errors(
    task: impl Future<Output = Result<TaskOutput, ()>>,
    abort_tx: mpsc::UnboundedSender<()>,
) -> Result<TaskOutput, ()> {
    AssertUnwindSafe(task)
        .catch_unwind()
        .await
        .map_err(|_| ())
        .and_then(|res| res)
        .map_err(|_| {
            error!("An error occurred that vector couldn't handle.");
            let _ = abort_tx.send(());
        })
}

/// If the closure returns false, then the element is removed
fn retain<T>(vec: &mut Vec<T>, mut retain_filter: impl FnMut(&mut T) -> bool) {
    let mut i = 0;
    while let Some(data) = vec.get_mut(i) {
        if retain_filter(data) {
            i += 1;
        } else {
            let _ = vec.remove(i);
        }
    }
}
