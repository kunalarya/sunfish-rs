use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub struct ArcWriter<T> {
    pub wrapped: Arc<T>,
}

unsafe impl<T> Send for ArcWriter<T> {}
unsafe impl<T> Sync for ArcWriter<T> {}

pub struct ArcReader<T> {
    pub wrapped: Arc<T>,
}

impl<T> fmt::Pointer for ArcWriter<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.wrapped, f)
    }
}

unsafe impl<T> Send for ArcReader<T> {}
unsafe impl<T> Sync for ArcReader<T> {}

impl<T> Deref for ArcWriter<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &(*self.wrapped)
    }
}
impl<T> DerefMut for ArcWriter<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { Arc::get_mut_unchecked(&mut self.wrapped) }
    }
}

impl<T> Deref for ArcReader<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &(*self.wrapped)
    }
}

/*
impl<T> Clone for ArcReader<T> {
    fn clone(&self) -> Self {
        ArcReader {
            wrapped: Arc::clone(&self.wrapped),
        }
    }
}
*/

impl<T> ArcReader<T> {
    pub fn clone(reader: &ArcReader<T>) -> Self {
        ArcReader {
            wrapped: Arc::clone(&reader.wrapped),
        }
    }
}

impl<T> fmt::Pointer for ArcReader<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.wrapped, f)
    }
}

/// Creates a new writer and cloneable reader.
pub fn new<T>(obj: T) -> (ArcWriter<T>, ArcReader<T>) {
    let p: Arc<T> = Arc::new(obj);
    let reader = ArcReader {
        wrapped: Arc::clone(&p),
    };
    let writer = ArcWriter {
        wrapped: Arc::clone(&p),
    };
    (writer, reader)
}
