//! The Queue the daemon holds (0.11, Queue; Stage 3b, ts-0066-zb): every decision that waits on the person, kept in
//! the store so a restart does not lose one. A permission request, a usage limit, a budget and a disk pause are its
//! items in 3b.
//!
//! Items are written to the store before they are in the list, and taken out of the store before they leave it, so
//! the list a client reads is never ahead of what a restart would find. A write that fails is logged and the item
//! still shows: a Queue that hid a decision because the disk was full would be worse than one that forgot it at the
//! next restart.

use crate::store::Store;
use collide_core::owner::{QueueChanged, QueueItem, QueueKind};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Told of each change to the set of items ([`Queue::on_change`]). It runs while the Queue's lock is held, so that two
/// changes are told in the order they were made: it must be quick, and must not call the Queue.
pub type QueueSink = Arc<dyn Fn(QueueChanged) + Send + Sync>;

pub struct Queue {
    store: Arc<Store>,
    items: Mutex<Vec<QueueItem>>,
    log: Arc<dyn Fn(&str) + Send + Sync>,
    sink: Mutex<Option<QueueSink>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Queue {
    /// The Queue as the store holds it. A stored item that is not readable is logged and left out.
    pub fn load(store: Arc<Store>, log: Arc<dyn Fn(&str) + Send + Sync>) -> Queue {
        let mut items = Vec::new();
        match store.queue_items() {
            Ok(rows) => {
                for (id, json) in rows {
                    match serde_json::from_str::<QueueItem>(&json) {
                        Ok(item) => items.push(item),
                        Err(error) => log(&format!("queue item {id} is not readable: {error}")),
                    }
                }
            }
            Err(error) => log(&format!("the queue could not be read: {error}")),
        }
        Queue {
            store,
            items: Mutex::new(items),
            log,
            sink: Mutex::new(None),
        }
    }

    /// Tells `sink` of each item that comes and each that goes, from now on. It replaces the sink there was.
    pub fn on_change(&self, sink: QueueSink) {
        *locked(&self.sink) = Some(sink);
    }

    /// Tells the sink that `added` or `removed` changed the set, with the size it has now. The caller holds the items'
    /// lock, so the sizes are in the order of the changes.
    fn tell(&self, items: &[QueueItem], added: Option<&str>, removed: Option<&str>) {
        let Some(sink) = locked(&self.sink).clone() else {
            return;
        };
        sink(QueueChanged {
            count: u32::try_from(items.len()).unwrap_or(u32::MAX),
            added: added.map(str::to_string).into_iter().collect(),
            removed: removed.map(str::to_string).into_iter().collect(),
        });
    }

    /// Adds an item, or replaces the one with the same id.
    pub fn push(&self, item: QueueItem) {
        match serde_json::to_string(&item) {
            Ok(json) => {
                if let Err(error) = self
                    .store
                    .set_queue_item(&item.id, item.run.as_deref(), &json)
                {
                    (self.log)(&format!("queue item {} was not stored: {error}", item.id));
                }
            }
            Err(error) => (self.log)(&format!("queue item {} is not JSON: {error}", item.id)),
        }
        let mut items = locked(&self.items);
        match items.iter_mut().find(|held| held.id == item.id) {
            Some(held) => *held = item,
            None => {
                let id = item.id.clone();
                items.push(item);
                self.tell(&items, Some(&id), None);
            }
        }
    }

    /// Takes an item out and returns it, or `None` when it is not there. Only the caller that gets `Some` answered
    /// the item, so two answers to one item never both act.
    pub fn take(&self, id: &str) -> Option<QueueItem> {
        let item = {
            let mut items = locked(&self.items);
            let at = items.iter().position(|item| item.id == id)?;
            let item = items.remove(at);
            self.tell(&items, None, Some(id));
            item
        };
        if let Err(error) = self.store.delete_queue_item(id) {
            (self.log)(&format!("queue item {id} was not deleted: {error}"));
        }
        Some(item)
    }

    /// Puts an item back, for an answer that could not be carried out.
    pub fn restore(&self, item: QueueItem) {
        self.push(item);
    }

    /// The items, those of `run` when it is given, oldest first.
    pub fn list(&self, run: Option<&str>) -> Vec<QueueItem> {
        let mut found: Vec<QueueItem> = locked(&self.items)
            .iter()
            .filter(|item| run.is_none_or(|run| item.run.as_deref() == Some(run)))
            .cloned()
            .collect();
        found.sort_by_key(|item| item.since_ms);
        found
    }

    pub fn get(&self, id: &str) -> Option<QueueItem> {
        locked(&self.items)
            .iter()
            .find(|item| item.id == id)
            .cloned()
    }

    /// Takes out every item of `run`, or only those of `kind`.
    pub fn clear_run(&self, run: &str, kind: Option<QueueKind>) {
        let ids: Vec<String> = locked(&self.items)
            .iter()
            .filter(|item| {
                item.run.as_deref() == Some(run) && kind.is_none_or(|kind| item.kind == kind)
            })
            .map(|item| item.id.clone())
            .collect();
        for id in ids {
            self.take(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A folder made for one test, in the system's temp folder. `Drop` removes the files in it by their own paths and
    /// then the folder (never a recursive delete).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> TempDir {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir =
                std::env::temp_dir().join(format!("collide-queue-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            if let Ok(entries) = fs::read_dir(&self.0) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(entry.path());
                }
            }
            let _ = fs::remove_dir(&self.0);
        }
    }

    fn item(id: &str, run: &str, kind: QueueKind, since_ms: u64) -> QueueItem {
        QueueItem {
            id: id.to_string(),
            kind,
            run: Some(run.to_string()),
            phase: None,
            task: None,
            since_ms,
            title: "a decision".to_string(),
            permission: None,
            untrusted_input: false,
            cache: None,
            reset_ms: None,
            show: Vec::new(),
            answers: Vec::new(),
        }
    }

    fn open(dir: &TempDir) -> Queue {
        let store = Arc::new(Store::open(&dir.0).unwrap());
        Queue::load(store, Arc::new(|_| {}))
    }

    #[test]
    fn items_are_listed_oldest_first_and_by_run() {
        let dir = TempDir::new();
        let queue = open(&dir);
        queue.push(item("b", "run-1", QueueKind::UsageLimit, 20));
        queue.push(item("a", "run-1", QueueKind::DiskFull, 10));
        queue.push(item("c", "run-2", QueueKind::DiskFull, 5));
        let ids = |items: Vec<QueueItem>| items.into_iter().map(|i| i.id).collect::<Vec<_>>();
        assert_eq!(ids(queue.list(None)), ["c", "a", "b"]);
        assert_eq!(ids(queue.list(Some("run-1"))), ["a", "b"]);
    }

    #[test]
    fn a_pushed_item_with_a_known_id_replaces_it() {
        let dir = TempDir::new();
        let queue = open(&dir);
        queue.push(item("a", "run-1", QueueKind::UsageLimit, 10));
        let mut newer = item("a", "run-1", QueueKind::UsageLimit, 10);
        newer.reset_ms = Some(7);
        queue.push(newer);
        assert_eq!(queue.list(None).len(), 1);
        assert_eq!(queue.get("a").unwrap().reset_ms, Some(7));
    }

    #[test]
    fn an_item_can_be_taken_once() {
        let dir = TempDir::new();
        let queue = open(&dir);
        queue.push(item("a", "run-1", QueueKind::UsageLimit, 10));
        assert!(queue.take("a").is_some());
        assert!(queue.take("a").is_none());
        assert!(queue.list(None).is_empty());
    }

    #[test]
    fn items_survive_a_reopened_store_and_taken_ones_do_not() {
        let dir = TempDir::new();
        {
            let queue = open(&dir);
            queue.push(item("kept", "run-1", QueueKind::UsageLimit, 10));
            queue.push(item("gone", "run-1", QueueKind::DiskFull, 20));
            queue.take("gone");
        }
        let queue = open(&dir);
        assert_eq!(queue.list(None).len(), 1);
        assert!(queue.get("kept").is_some());
    }

    #[test]
    fn clearing_a_run_takes_its_items_or_those_of_one_kind() {
        let dir = TempDir::new();
        let queue = open(&dir);
        queue.push(item("a", "run-1", QueueKind::UsageLimit, 1));
        queue.push(item("b", "run-1", QueueKind::DiskFull, 2));
        queue.push(item("c", "run-2", QueueKind::DiskFull, 3));
        queue.clear_run("run-1", Some(QueueKind::DiskFull));
        assert!(queue.get("b").is_none());
        assert!(queue.get("a").is_some());
        queue.clear_run("run-1", None);
        assert!(queue.get("a").is_none());
        assert!(queue.get("c").is_some());
    }

    fn changes(queue: &Queue) -> Arc<Mutex<Vec<QueueChanged>>> {
        let told = Arc::new(Mutex::new(Vec::new()));
        let sink = told.clone();
        queue.on_change(Arc::new(move |changed| sink.lock().unwrap().push(changed)));
        told
    }

    fn change(count: u32, added: &[&str], removed: &[&str]) -> QueueChanged {
        let names = |ids: &[&str]| ids.iter().map(|id| id.to_string()).collect();
        QueueChanged {
            count,
            added: names(added),
            removed: names(removed),
        }
    }

    #[test]
    fn a_new_item_is_told_as_added_with_the_size_after_it() {
        let dir = TempDir::new();
        let queue = open(&dir);
        let told = changes(&queue);
        queue.push(item("a", "run-1", QueueKind::UsageLimit, 1));
        queue.push(item("b", "run-1", QueueKind::DiskFull, 2));
        assert_eq!(
            *told.lock().unwrap(),
            [change(1, &["a"], &[]), change(2, &["b"], &[])]
        );
    }

    #[test]
    fn a_taken_item_is_told_as_removed_and_taking_it_again_tells_nothing() {
        let dir = TempDir::new();
        let queue = open(&dir);
        queue.push(item("a", "run-1", QueueKind::UsageLimit, 1));
        let told = changes(&queue);
        queue.take("a");
        queue.take("a");
        assert_eq!(*told.lock().unwrap(), [change(0, &[], &["a"])]);
    }

    #[test]
    fn a_replaced_item_changes_no_set_and_tells_nothing() {
        let dir = TempDir::new();
        let queue = open(&dir);
        queue.push(item("a", "run-1", QueueKind::UsageLimit, 1));
        let told = changes(&queue);
        queue.push(item("a", "run-1", QueueKind::UsageLimit, 1));
        assert!(told.lock().unwrap().is_empty());
    }

    #[test]
    fn clearing_a_run_tells_each_item_that_went() {
        let dir = TempDir::new();
        let queue = open(&dir);
        queue.push(item("a", "run-1", QueueKind::UsageLimit, 1));
        queue.push(item("b", "run-1", QueueKind::DiskFull, 2));
        queue.push(item("c", "run-2", QueueKind::DiskFull, 3));
        let told = changes(&queue);
        queue.clear_run("run-1", None);
        assert_eq!(
            *told.lock().unwrap(),
            [change(2, &[], &["a"]), change(1, &[], &["b"])]
        );
    }

    #[test]
    fn a_restored_item_is_back_in_the_list() {
        let dir = TempDir::new();
        let queue = open(&dir);
        queue.push(item("a", "run-1", QueueKind::UsageLimit, 1));
        let taken = queue.take("a").unwrap();
        queue.restore(taken);
        assert!(queue.get("a").is_some());
    }
}
