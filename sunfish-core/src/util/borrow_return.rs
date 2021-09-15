/// A autoborrow lets you borrow an owned object and automatically
/// return it back when the borrow is dropped.
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct Owner<T> {
    val: Arc<Mutex<Option<T>>>,
}

impl<T> Owner<T> {
    pub fn new(val: T) -> Self {
        Owner {
            val: Arc::new(Mutex::new(Some(val))),
        }
    }

    fn clone_ref(&self) -> Self {
        Self {
            val: Arc::clone(&self.val),
        }
    }

    pub fn borrow(&mut self) -> Borrower<T> {
        let taken = self.val.lock().unwrap().take().unwrap();
        Borrower::new(taken, self.clone_ref())
    }
}

#[derive(Debug)]
pub struct Borrower<T> {
    pub grabbed: Option<T>,
    owner: Owner<T>,
}

impl<T> Drop for Borrower<T> {
    fn drop(&mut self) {
        let item = self.grabbed.take().unwrap();
        (*self.owner.val.lock().unwrap()) = Some(item);
    }
}

impl<T> Borrower<T> {
    fn new(val: T, owner: Owner<T>) -> Self {
        Borrower {
            grabbed: Some(val),
            owner,
        }
    }
}
