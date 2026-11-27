//! The event stream's switchboard (ts-0076-cw; 0.2, The wire): which connections have subscribed, and the notices that
//! reach them. The daemon's door for events ([`crate::phases::Phases::append_event`]) and the Queue
//! ([`crate::queue::Queue`]) call [`Hub::publish_event`] and [`Hub::publish_queue`]; a connection's thread
//! calls [`Hub::register`] for `daemon.subscribe` and [`Hub::remove`] when it ends.
//!
//! - **A publisher never waits.** Each subscriber has a bounded queue, and a publish is a `try_send` on it. A
//!   subscriber whose queue is full is ended and logged, and nothing else is held up. The queue is read by a thread of
//!   the connection's own ([`crate::daemon`]), which writes to the socket, so a client that reads slowly fills its
//!   queue and a client that reads nothing fills it for good.
//! - **Order is the publisher's.** Events are published under the lock that numbers them, after the store has them, so
//!   each queue holds events in `seq` order. A queue may hold an event the store replay also gave: the reader drops it
//!   by its `seq`.
//! - **One subscription for each connection.** Registering again cancels the earlier subscription, so its reader stops
//!   at once and sends nothing more.

use collide_core::events::Event;
use collide_core::owner::{BoardChanged, Notice, QueueChanged};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// What a subscribed connection reads: its queue, and the two flags the hub sets. `cancelled` says the subscription was
/// replaced or its connection ended. `overflowed` says the queue was full and the hub ended the subscription: the
/// reader drains what the queue holds, finds it closed, and the connection is closed.
pub struct Subscription {
    pub notices: Receiver<Notice>,
    pub cancelled: Arc<AtomicBool>,
    pub overflowed: Arc<AtomicBool>,
}

/// A subscriber's side of its queue. `sender` is `None` once the queue was full or its reader was gone. The entry stays
/// until the connection's [`Hub::remove`], so a new [`Hub::register`] of the connection still cancels it.
struct Entry {
    sender: Option<SyncSender<Notice>>,
    cancelled: Arc<AtomicBool>,
    overflowed: Arc<AtomicBool>,
}

pub struct Hub {
    capacity: usize,
    log: Arc<dyn Fn(&str) + Send + Sync>,
    subscribers: Mutex<HashMap<u64, Entry>>,
}

impl Hub {
    /// A hub whose subscribers each hold up to `capacity` notices (at least one).
    pub fn new(capacity: usize, log: Arc<dyn Fn(&str) + Send + Sync>) -> Hub {
        Hub {
            capacity: capacity.max(1),
            log,
            subscribers: Mutex::new(HashMap::new()),
        }
    }

    fn held(&self) -> MutexGuard<'_, HashMap<u64, Entry>> {
        self.subscribers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Subscribes `connection`, and cancels the subscription it had.
    pub fn register(&self, connection: u64) -> Subscription {
        let (sender, notices) = sync_channel(self.capacity);
        let cancelled = Arc::new(AtomicBool::new(false));
        let overflowed = Arc::new(AtomicBool::new(false));
        let entry = Entry {
            sender: Some(sender),
            cancelled: cancelled.clone(),
            overflowed: overflowed.clone(),
        };
        if let Some(earlier) = self.held().insert(connection, entry) {
            earlier.cancelled.store(true, Ordering::SeqCst);
        }
        Subscription {
            notices,
            cancelled,
            overflowed,
        }
    }

    /// Ends `connection`'s subscription, if it has one.
    pub fn remove(&self, connection: u64) {
        if let Some(entry) = self.held().remove(&connection) {
            entry.cancelled.store(true, Ordering::SeqCst);
        }
    }

    /// Sends the event to every subscriber, as a `daemon.event` notice.
    pub fn publish_event(&self, event: &Event) {
        self.publish(&Notice::DaemonEvent(Box::new(event.clone())));
    }

    /// Sends the change to every subscriber, as a `queue.changed` notice.
    pub fn publish_queue(&self, changed: QueueChanged) {
        self.publish(&Notice::QueueChanged(changed));
    }

    /// Sends the change to every subscriber, as a `board.changed` notice (ts-0222-gx).
    pub fn publish_board(&self, changed: BoardChanged) {
        self.publish(&Notice::BoardChanged(changed));
    }

    /// How many connections have a subscription that is still being sent to.
    pub fn subscribers(&self) -> usize {
        self.held()
            .values()
            .filter(|entry| entry.sender.is_some())
            .count()
    }

    /// A `try_send` to each queue. A full queue ends its subscription: its sender is dropped, so its reader finds the
    /// queue closed after the notices it holds. A queue whose reader is gone is dropped the same way.
    fn publish(&self, notice: &Notice) {
        for (connection, entry) in self.held().iter_mut() {
            let Some(sender) = &entry.sender else {
                continue;
            };
            match sender.try_send(notice.clone()) {
                Ok(()) => {}
                Err(TrySendError::Disconnected(_)) => entry.sender = None,
                Err(TrySendError::Full(_)) => {
                    (self.log)(&format!(
                        "connection {connection} fell {} notices behind and its subscription was ended",
                        self.capacity
                    ));
                    entry.overflowed.store(true, Ordering::SeqCst);
                    entry.sender = None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use collide_core::change::{Actor, ActorKind};
    use collide_core::events::{EventBody, StepKind};
    use std::sync::mpsc::TryRecvError;

    fn event(seq: u64) -> Event {
        Event {
            seq,
            wall_ms: seq,
            actor: Actor {
                kind: ActorKind::Workflow,
                run: None,
                phase: None,
            },
            experiment: None,
            body: EventBody::StepIntent {
                step: "s".to_string(),
                kind: StepKind::Integrate,
                action: "a".to_string(),
            },
        }
    }

    fn hub(capacity: usize) -> (Hub, Arc<Mutex<Vec<String>>>) {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let sink = lines.clone();
        let hub = Hub::new(
            capacity,
            Arc::new(move |text: &str| sink.lock().unwrap().push(text.to_string())),
        );
        (hub, lines)
    }

    fn seqs(subscription: &Subscription) -> Vec<u64> {
        let mut seqs = Vec::new();
        while let Ok(notice) = subscription.notices.try_recv() {
            if let Notice::DaemonEvent(event) = notice {
                seqs.push(event.seq);
            }
        }
        seqs
    }

    #[test]
    fn each_subscriber_gets_the_events_in_the_order_they_were_published() {
        let (hub, _) = hub(8);
        let (one, two) = (hub.register(1), hub.register(2));
        for seq in 1..=3 {
            hub.publish_event(&event(seq));
        }
        assert_eq!(seqs(&one), [1, 2, 3]);
        assert_eq!(seqs(&two), [1, 2, 3]);
    }

    #[test]
    fn a_queue_change_goes_to_each_subscriber() {
        let (hub, _) = hub(8);
        let one = hub.register(1);
        let changed = QueueChanged {
            count: 1,
            added: vec!["a".to_string()],
            removed: vec![],
        };
        hub.publish_queue(changed.clone());
        assert_eq!(
            one.notices.try_recv().unwrap(),
            Notice::QueueChanged(changed)
        );
    }

    #[test]
    fn nothing_published_before_a_registration_reaches_it() {
        let (hub, _) = hub(8);
        hub.publish_event(&event(1));
        let one = hub.register(1);
        assert!(matches!(one.notices.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn a_subscriber_whose_queue_is_full_is_taken_out_and_logged_and_the_others_go_on() {
        let (hub, lines) = hub(2);
        let (slow, quick) = (hub.register(1), hub.register(2));
        for seq in 1..=4 {
            hub.publish_event(&event(seq));
            assert_eq!(seqs(&quick), [seq]);
        }
        assert_eq!(hub.subscribers(), 1);
        assert!(slow.overflowed.load(Ordering::SeqCst));
        assert!(!quick.overflowed.load(Ordering::SeqCst));
        assert_eq!(
            seqs(&slow),
            [1, 2],
            "it keeps the prefix it had, with no gap"
        );
        assert!(matches!(
            slow.notices.try_recv(),
            Err(TryRecvError::Disconnected)
        ));
        assert!(
            lines
                .lock()
                .unwrap()
                .iter()
                .any(|l| l.contains("connection 1")),
            "{:?}",
            lines.lock().unwrap()
        );
    }

    #[test]
    fn registering_again_cancels_the_earlier_subscription_and_starts_a_new_queue() {
        let (hub, _) = hub(8);
        let first = hub.register(1);
        hub.publish_event(&event(1));
        let second = hub.register(1);
        hub.publish_event(&event(2));
        assert!(first.cancelled.load(Ordering::SeqCst));
        assert!(!second.cancelled.load(Ordering::SeqCst));
        assert_eq!(seqs(&second), [2]);
        assert_eq!(hub.subscribers(), 1);
    }

    #[test]
    fn registering_again_after_an_overflow_still_cancels_the_ended_subscription() {
        let (hub, _) = hub(1);
        let first = hub.register(1);
        hub.publish_event(&event(1));
        hub.publish_event(&event(2));
        assert!(first.overflowed.load(Ordering::SeqCst));
        let second = hub.register(1);
        assert!(first.cancelled.load(Ordering::SeqCst));
        hub.publish_event(&event(3));
        assert_eq!(seqs(&second), [3]);
    }

    #[test]
    fn a_removed_connection_gets_nothing_more_and_is_cancelled() {
        let (hub, _) = hub(8);
        let one = hub.register(1);
        hub.remove(1);
        hub.publish_event(&event(1));
        assert!(one.cancelled.load(Ordering::SeqCst));
        assert_eq!(hub.subscribers(), 0);
        assert!(seqs(&one).is_empty());
        hub.remove(1);
    }
}
