use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use eframe::egui;
use parking_lot::RwLock;
use slog::{Logger, debug, o};
use tokio_stream::StreamExt as _;
use tokio_util::time::{DelayQueue, delay_queue::Key as DelayKey};
use tracing::Instrument as _;

use super::handle_event;
use crate::{AppState, Msg};

pub async fn handle(
    mut rx: tokio::sync::mpsc::Receiver<Msg>,
    logger: Logger,
    state: Arc<RwLock<AppState>>,
) {
    let mut local_queue: VecDeque<Msg> = VecDeque::new();
    let mut delay_queue: DelayQueue<(egui::Id, Msg)> = DelayQueue::new();
    let mut pending_debounces: HashMap<egui::Id, DelayKey> = HashMap::new();
    let mut cleanup_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    let logger = logger.new(o!("category" => "event"));

    loop {
        tokio::select! {
            biased;
            _ = cleanup_interval.tick() => {
                local_queue.push_back(Msg::CheckDependencies);
            }

            Some(expired) = delay_queue.next(), if !delay_queue.is_empty() => {
                let (id, msg) = expired.into_inner();
                pending_debounces.remove(&id);
                local_queue.push_back(msg);
            }
            maybe_msg = rx.recv() => {
                match maybe_msg {
                    Some(Msg::Debounce(dur, id, inner)) => {
                        if let Some(delay_key) = pending_debounces.get(&id) {
                            delay_queue.remove(delay_key);
                        }
                        let queue_key = delay_queue.insert_at(
                            (id, *inner),
                            tokio::time::Instant::now() + dur,
                        );
                        pending_debounces.insert(id, queue_key);

                    }
                    Some(msg) => local_queue.push_back(msg),
                    None => break,
                }
            }

        };
        while let Some(msg) = local_queue.pop_front() {
            #[cfg(feature = "profile")]
            {
                tracing::info!(
                    tracy.plot = "Event Local Queue Size",
                    value = local_queue.len() as f64
                );
            }
            let span = tracing::info_span!("handle_event", kind = ?std::mem::discriminant(&msg));
            let _ = handle_event(logger.clone(), msg, state.clone(), &mut local_queue)
                .instrument(span)
                .await;
        }
    }
    debug!(logger, "Handler exiting");
}
