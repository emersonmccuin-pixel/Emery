use std::sync::{Condvar, Mutex};
use std::time::Duration;

#[derive(Default)]
pub struct AgentMessageBroker {
    sequence: Mutex<u64>,
    changed: Condvar,
}

impl AgentMessageBroker {
    pub fn current_sequence(&self) -> u64 {
        *self
            .sequence
            .lock()
            .expect("agent message broker mutex poisoned")
    }

    pub fn notify_message(&self) {
        let mut sequence = self
            .sequence
            .lock()
            .expect("agent message broker mutex poisoned");
        *sequence = sequence.saturating_add(1);
        self.changed.notify_all();
    }

    pub fn wait_for_change(&self, observed_sequence: u64, timeout: Duration) -> bool {
        let sequence = self
            .sequence
            .lock()
            .expect("agent message broker mutex poisoned");

        if *sequence != observed_sequence {
            return true;
        }

        let (sequence, result) = self
            .changed
            .wait_timeout_while(sequence, timeout, |current| *current == observed_sequence)
            .expect("agent message broker mutex poisoned");

        *sequence != observed_sequence || !result.timed_out()
    }
}
