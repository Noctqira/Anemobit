use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::traffic::Clock;

/// Virtual clock: sleep only advances virtual time and records the duration, without actually waiting. Clones share state.
#[derive(Clone)]
pub(crate) struct TestClock {
    state: Rc<RefCell<State>>,
}

struct State {
    current: Instant,
    sleeps: Vec<Duration>,
}

impl Default for TestClock {
    fn default() -> Self {
        TestClock {
            state: Rc::new(RefCell::new(State {
                current: Instant::now(),
                sleeps: Vec::new(),
            })),
        }
    }
}

impl TestClock {
    /// Duration of each sleep.
    pub(crate) fn sleeps(&self) -> Vec<Duration> {
        self.state.borrow().sleeps.clone()
    }
}

impl Clock for TestClock {
    fn now(&self) -> Instant {
        self.state.borrow().current
    }

    async fn sleep(&self, duration: Duration) {
        let mut state = self.state.borrow_mut();
        state.current += duration;
        state.sleeps.push(duration);
    }
}
