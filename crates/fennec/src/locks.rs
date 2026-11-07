//! Lock helpers that recover from poisoned locks.
//!
//! The data behind fennec's locks are caches and maps of handles that stay
//! usable after a panic; refusing them would turn one bug into a dead server.

use std::sync::{Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn write<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn poisoned_locks_stay_usable() {
        let mutex = Arc::new(Mutex::new(42));
        let mutex_for_panic = Arc::clone(&mutex);
        let thread = std::thread::spawn(move || {
            let _guard = mutex_for_panic.lock().unwrap();
            panic!("poison the mutex");
        });
        let _ = thread.join();
        assert_eq!(*lock(&mutex), 42);

        let rwlock = Arc::new(RwLock::new(7));
        let rwlock_for_panic = Arc::clone(&rwlock);
        let thread = std::thread::spawn(move || {
            let _guard = rwlock_for_panic.write().unwrap();
            panic!("poison the rwlock");
        });
        let _ = thread.join();
        assert_eq!(*read(&rwlock), 7);
        assert_eq!(*write(&rwlock), 7);
    }
}
