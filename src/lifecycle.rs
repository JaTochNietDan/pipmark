//! Independent suspension reasons: a display wake must not override session inactivity.
#[derive(Default)]
pub struct Lifecycle {
    reasons: u8,
}
#[derive(Clone, Copy)]
pub enum Reason {
    System = 1,
    Display = 2,
    Session = 4,
}
impl Lifecycle {
    pub fn set(&mut self, reason: Reason, suspended: bool) {
        if suspended {
            self.reasons |= reason as u8;
        } else {
            self.reasons &= !(reason as u8);
        }
    }
    pub fn suspended(&self) -> bool {
        self.reasons != 0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_events_resume_only_when_all_reasons_clear() {
        let mut state = Lifecycle::default();
        state.set(Reason::Display, true);
        state.set(Reason::Session, true);
        state.set(Reason::Display, false);
        assert!(state.suspended());
        state.set(Reason::System, true);
        state.set(Reason::Session, false);
        assert!(state.suspended());
        state.set(Reason::System, false);
        assert!(!state.suspended());
    }
    #[test]
    fn duplicate_events_are_idempotent() {
        let mut state = Lifecycle::default();
        state.set(Reason::System, true);
        state.set(Reason::System, true);
        state.set(Reason::System, false);
        assert!(!state.suspended());
    }
}
