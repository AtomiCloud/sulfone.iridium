//! Simple semaphore for limiting parallel test execution.

use std::sync::{Arc, Condvar, Mutex};

pub struct Semaphore {
    permits: Arc<Mutex<usize>>,
    condvar: Arc<Condvar>,
}

impl Semaphore {
    pub fn new(permits: usize) -> Self {
        Semaphore {
            permits: Arc::new(Mutex::new(permits)),
            condvar: Arc::new(Condvar::new()),
        }
    }

    pub fn acquire(&self) -> SemaphorePermit<'_> {
        let mut available = self.permits.lock().unwrap();
        while *available == 0 {
            available = self.condvar.wait(available).unwrap();
        }
        *available -= 1;
        SemaphorePermit { semaphore: self }
    }
}

pub struct SemaphorePermit<'a> {
    semaphore: &'a Semaphore,
}

impl<'a> Drop for SemaphorePermit<'a> {
    fn drop(&mut self) {
        let mut available = self.semaphore.permits.lock().unwrap();
        *available += 1;
        self.semaphore.condvar.notify_one();
    }
}
