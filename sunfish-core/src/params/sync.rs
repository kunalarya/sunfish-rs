use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use copy_from::CopyFrom;
use crossbeam::atomic::AtomicCell;

use crate::params::{EParam, NormalizedParams, Params, ParamsMeta};

pub type EnqueuedParams = HashMap<EParam, f64>;

#[derive(Clone)]
pub struct Subscriber {
    // Parameters that have changed.
    pub changes: Arc<Mutex<EnqueuedParams>>,
    // Last acknowledged epoch.
    last_epoch: Arc<AtomicU32>,
}

pub struct Synchronizer {
    pub meta: ParamsMeta,

    /// Authoritative source of parameters, along with epoch number.
    pub params: Arc<Mutex<(Params, u32)>>,
    /// Local copy in case we cannot get the mutex lock. Of course, this is *not* consistent.
    params_copy: Params,

    /// Parameters that need to be written to the authoritative, useful for when we cannot
    /// get the mutex lock.
    on_deck: EnqueuedParams,

    /// Store mailboxes & subscribers in the same mutex:
    /// - Mailboxes get a copy of the parameters, along with changes
    ///   included in that copy.
    /// - Subscribers only get the changes. They are responsible for
    ///   synchronizing that information.
    #[allow(clippy::type_complexity)]
    mailboxes_and_subs: Arc<Mutex<(Vec<MailboxWriter<(Params, u32)>>, Vec<Subscriber>)>>,
}

impl std::clone::Clone for Synchronizer {
    fn clone(&self) -> Self {
        Self {
            meta: self.meta.clone(),
            params: Arc::clone(&self.params),
            params_copy: self.params_copy.clone(),
            on_deck: HashMap::new(),
            mailboxes_and_subs: Arc::clone(&self.mailboxes_and_subs),
        }
    }
}

impl Synchronizer {
    pub fn new(meta: ParamsMeta, params: Params) -> Self {
        let params_copy = params.clone();
        Synchronizer {
            meta,
            params: Arc::new(Mutex::new((params, 0))),
            params_copy,
            mailboxes_and_subs: Arc::new(Mutex::new((vec![], vec![]))),
            on_deck: HashMap::new(),
        }
    }

    pub fn subscriber(&mut self) -> Subscriber {
        // TODO: This should go in the
        let subscriber = Subscriber {
            changes: Arc::new(Mutex::new(HashMap::new())),
            last_epoch: Arc::new(AtomicU32::new(0)),
        };
        let (_mailboxes, subscribers) = &mut (*self
            .mailboxes_and_subs
            .lock()
            .expect("Access mailboxes and subscribers"));
        subscribers.push(Subscriber {
            changes: Arc::clone(&subscriber.changes),
            last_epoch: Arc::clone(&subscriber.last_epoch),
        });
        // TODO: Return subscriber client, tie locking to other mutex.
        subscriber
    }

    pub fn mailbox(&mut self) -> MailboxReceiver {
        let last_epoch = Arc::new(AtomicU32::new(0));
        let changes = Arc::new(Mutex::new(HashMap::new()));
        let (mailbox_writer, mailbox_reader) = mailbox();
        let reader = MailboxReceiver {
            reader: mailbox_reader,
            subscriber: Subscriber {
                changes,
                last_epoch: Arc::clone(&last_epoch),
            },
        };
        let (mailboxes, subscribers) = &mut (*self
            .mailboxes_and_subs
            .lock()
            .expect("Access mailboxes and subscribers"));
        mailboxes.push(mailbox_writer);
        subscribers.push(reader.subscriber.clone());
        reader
    }

    pub fn write_parameter(&mut self, eparam: EParam, value: f64) {
        if let Ok(mut guard) = self.params.try_lock() {
            let (shared_params, epoch) = &mut *guard;

            let (mailboxes, subscribers) = &mut (*self
                .mailboxes_and_subs
                .lock()
                .expect("Access mailboxes and subscribers"));
            for subscriber in (*subscribers).iter_mut() {
                if let Ok(mut guard) = subscriber.changes.try_lock() {
                    let changes = &mut (*guard);
                    // Reset the shared queue when we have the lock, before
                    // we add any new updates to it.
                    if subscriber.last_epoch.load(Ordering::Acquire) >= *epoch {
                        changes.clear();
                    }
                    for (enq_param, enq_value) in &self.on_deck {
                        changes.insert(*enq_param, *enq_value);
                    }
                    changes.insert(eparam, value);
                }
            }
            *epoch += 1;
            // Apply all "on deck" changes.
            for (enq_param, enq_value) in self.on_deck.drain() {
                shared_params.write_parameter(&self.meta, enq_param, enq_value);
            }
            // Finally write the parameter we intend to write.
            shared_params.write_parameter(&self.meta, eparam, value);
            // Since we have access to the parameters, we take the opportunity to refresh our view
            // of parameters.
            self.params_copy.copy_from(shared_params);
            for mailbox in mailboxes {
                let next = guard.clone();
                mailbox.update(next);
            }
        } else {
            self.on_deck.insert(eparam, value);
        }
    }

    pub fn refresh_maybe(&mut self) {
        if let Ok(guard) = self.params.try_lock() {
            let (shared_params, _shared_queue) = &*guard;
            // Since we have access to the parameters, we take the opportunity to refresh our view
            // of parameters.
            self.params_copy.copy_from(shared_params);
        }
    }

    pub fn refresh(&mut self) {
        if let Ok(guard) = self.params.lock() {
            let (shared_params, _shared_queue) = &*guard;
            // Since we have access to the parameters, we take the opportunity to refresh our view
            // of parameters.
            self.params_copy.copy_from(shared_params);
        }
    }

    pub fn read_parameter(&mut self, eparam: EParam) -> f64 {
        self.params_copy.read_parameter(&self.meta, eparam)
    }

    pub fn clone_inner(&self) -> Option<Params> {
        match self.params.lock() {
            Ok(guard) => {
                let (shared_params, _) = &*guard;
                Some(shared_params.clone())
            }
            Err(err) => {
                log::error!(
                    "clone_inner failed to get a lock; defaulting to sample rate: {:?}",
                    err
                );
                None
            }
        }
    }

    pub fn formatted_value(&self, eparam: EParam) -> String {
        self.params_copy.formatted_value(&self.meta, eparam)
    }
}

/// Exclusive parameter "reader"; this is designed for the core render
/// loop, used to query the authoritative source of parameters, copy it into
/// its local parameters, then flag to the synchronizer that it's up-to-date.
pub struct MailboxReceiver {
    reader: MailboxReader<(Params, u32)>,
    subscriber: Subscriber,
}

impl MailboxReceiver {
    pub fn check_and_update<F>(&self, last_epoch_recorded: &mut u32, update: F)
    where
        F: FnOnce(Params, &EnqueuedParams),
    {
        if let Some((params, epoch)) = self.reader.get_updated() {
            if epoch > *last_epoch_recorded {
                if let Ok(guard) = self.subscriber.changes.lock() {
                    let changes = &*guard;
                    update(params, changes);
                }
            }
            self.subscriber.last_epoch.store(epoch, Ordering::Release);
            *last_epoch_recorded = epoch;
        }
    }
}

/******************************************************************************
 ** Mailbox                                                                  **
 ******************************************************************************/

pub fn mailbox<T: Clone>() -> (MailboxWriter<T>, MailboxReader<T>) {
    let slot = Arc::new(AtomicCell::new(None));
    let ready = Arc::new(AtomicBool::new(false));

    (
        MailboxWriter {
            slot: Arc::clone(&slot),
            ready: Arc::clone(&ready),
        },
        MailboxReader { slot, ready },
    )
}

#[derive(Clone)]
pub struct MailboxWriter<T: Clone> {
    slot: Arc<AtomicCell<Option<T>>>,
    ready: Arc<AtomicBool>,
}

impl<T: Clone> MailboxWriter<T> {
    fn update(&self, next: T) {
        self.slot.store(Some(next));
        self.ready.store(true, Ordering::Release);
    }
}

pub struct MailboxReader<T: Clone> {
    slot: Arc<AtomicCell<Option<T>>>,
    ready: Arc<AtomicBool>,
}

impl<T: Clone> MailboxReader<T> {
    pub fn get_updated(&self) -> Option<T> {
        // Check if there's a new value.
        if self.ready.swap(false, Ordering::Acquire) {
            // If so, swap it with None.
            self.slot.swap(None)
        } else {
            None
        }
    }
}
