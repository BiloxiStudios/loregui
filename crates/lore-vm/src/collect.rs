//! Shared event-stream collector.
//!
//! The lore crate's async fns emit events through a `LoreEventCallback`.
//! This module provides a callback + receiver pair that collects all events
//! until `Complete` or `Error` arrives, returning a typed `EventStream`.

use lore::interface::LoreEvent;
use lore::interface::LoreEventCallback;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// Collected events from a single lore call.
#[derive(Default)]
pub struct EventStream {
    pub events: Vec<LoreEvent>,
    pub status: Option<i32>,
    pub error: Option<String>,
}

impl EventStream {
    /// Return true if the call succeeded (status == 0 and no error).
    pub fn is_ok(&self) -> bool {
        self.status == Some(0) && self.error.is_none()
    }

    /// Find the first `AuthUserInfo` event and return (id, name).
    pub fn auth_user_info(&self) -> Option<(String, String)> {
        for event in &self.events {
            if let LoreEvent::AuthUserInfo(data) = event {
                return Some((data.id.as_str().into(), data.name.as_str().into()));
            }
        }
        None
    }
}

/// Create a callback + receiver that collects events until completion.
///
/// Returns `(callback, receiver)`. The receiver yields an `EventStream` once
/// the `Complete` event arrives.
pub fn collect_events() -> (LoreEventCallback, oneshot::Receiver<EventStream>) {
    let (tx, rx) = oneshot::channel();
    let stream: Arc<Mutex<EventStream>> = Arc::new(Mutex::new(EventStream::default()));
    // The lore callback is `Fn` (invoked once per streamed event), but a oneshot
    // Sender can only fire once and is consumed by `send`. Hold it in a shared
    // Option so the `Fn` closure can `take()` it exactly once on completion.
    let tx: Arc<Mutex<Option<oneshot::Sender<EventStream>>>> = Arc::new(Mutex::new(Some(tx)));

    let callback = Box::new(move |event: &LoreEvent| {
        let mut s = stream.lock().unwrap();
        match event {
            LoreEvent::Error(data) => {
                s.error = Some(data.error_inner.as_str().to_string());
            }
            LoreEvent::Complete(data) => {
                s.status = Some(data.status);
            }
            _ => {}
        }
        s.events.push(event.clone());

        // Signal once the terminal event (Complete or Error) arrives.
        let done = matches!(event, LoreEvent::Complete(_) | LoreEvent::Error(_));
        if done {
            let final_stream = std::mem::take(&mut *s);
            drop(s);
            if let Some(sender) = tx.lock().unwrap().take() {
                let _ = sender.send(final_stream);
            }
        }
    });

    (Some(callback), rx)
}
