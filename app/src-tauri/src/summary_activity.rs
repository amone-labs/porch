//! Tracks app-owned summary work independently of webview lifetime.
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Activity(AtomicUsize);
pub struct Guard<'a>(&'a Activity);

impl Activity {
    pub const fn new() -> Self { Self(AtomicUsize::new(0)) }
    pub fn start(&self) -> Guard<'_> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Guard(self)
    }
    pub fn running(&self) -> bool { self.0.load(Ordering::Relaxed) > 0 }
}
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.0.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_busy_until_last_summary_finishes() {
        let activity = Activity::new();
        assert!(!activity.running());
        let first = activity.start();
        let second = activity.start();
        assert!(activity.running());
        drop(first);
        assert!(activity.running());
        drop(second);
        assert!(!activity.running());
    }

    #[test]
    fn failure_and_unwind_release_activity() {
        let activity = Activity::new();
        let result: Result<(), &str> = {
            let _guard = activity.start();
            assert!(activity.running());
            Err("summary failed")
        };
        assert!(result.is_err());
        assert!(!activity.running());
        let _ = std::panic::catch_unwind(|| {
            let _guard = activity.start();
            panic!("worker panicked");
        });
        assert!(!activity.running());
    }
}
