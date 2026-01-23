use std::path::{Path};

use iced::{
    Subscription,
    stream as iced_stream,
    futures::channel::mpsc::Sender,
    futures::stream::{ BoxStream, StreamExt},
    futures::{SinkExt },
};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use crate::messages::Message;

pub fn file_watcher(path: &Path) -> Subscription<Message> {
    let s = path.to_string_lossy().into_owned();
    Subscription::run_with(s, watch_stream)
}
#[allow(clippy::ptr_arg)]
fn watch_stream(path: &String) -> BoxStream<'static, Message> {
    let owned_path = path.clone();
    Box::pin(iced_stream::channel(
        100,
        move |mut output: Sender<Message>| async move {
            let (mut tx, mut rx) = iced::futures::channel::mpsc::channel(10);
            let mut watcher = RecommendedWatcher::new(
                move |res| {
                    let _ = tx.try_send(res);
                },
                Config::default(),
            )
            .expect("Failed to create watcher");

            watcher
                .watch(Path::new(&owned_path), RecursiveMode::NonRecursive)
                .expect("Failed to watch path");

            while let Some(res) = rx.next().await {
                match res {
                    Ok(_event) => {
                        let _ = output.send(Message::LoadedFileChanged).await;
                    },
                    Err(e) => eprintln!("Watch error: {:?}", e),
                }
            }
        },
    ))
}
