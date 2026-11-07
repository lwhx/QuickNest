//! Panel visibility state machine (baseline chapter 10).
//!
//! Pure logic with no Win32 or Slint dependency so every transition and
//! suppression decision is unit-testable. Platform layers drive it and apply
//! the resulting window actions.

use std::fmt;
use std::sync::{Arc, Mutex};

/// Visibility states from baseline 10.1. Showing/Hiding are transient: the
/// transition methods walk through them and return the walked path so callers
/// can record the exact sequence as evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelState {
    Hidden,
    Showing,
    Visible,
    Hiding,
    ShuttingDown,
}

impl fmt::Display for PanelState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Hidden => "Hidden",
            Self::Showing => "Showing",
            Self::Visible => "Visible",
            Self::Hiding => "Hiding",
            Self::ShuttingDown => "ShuttingDown",
        };
        f.write_str(name)
    }
}

/// Suppression reasons from baseline 10.2. Every lock must have an explicit
/// release path; RAII tokens own their release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuppressionReason {
    Pinned,
    ModalDialogOpen,
    SystemDialogActive,
    ExternalDragHover,
    ContextMenuOpen,
    LaunchingElevated,
}

impl fmt::Display for SuppressionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Pinned => "Pinned",
            Self::ModalDialogOpen => "ModalDialogOpen",
            Self::SystemDialogActive => "SystemDialogActive",
            Self::ExternalDragHover => "ExternalDragHover",
            Self::ContextMenuOpen => "ContextMenuOpen",
            Self::LaunchingElevated => "LaunchingElevated",
        };
        f.write_str(name)
    }
}

#[derive(Debug)]
pub enum StateError {
    /// A transition was attempted from ShuttingDown; the panel is going away.
    ShuttingDown,
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShuttingDown => f.write_str("panel is shutting down"),
        }
    }
}

/// Reference-counted suppression registry. Tokens share this through an Arc
/// so a token still releases correctly even if the machine handle was dropped
/// first.
#[derive(Debug, Default)]
pub struct SuppressionSet {
    counts: Vec<(SuppressionReason, u64)>,
}

impl SuppressionSet {
    pub fn acquire(&mut self, reason: SuppressionReason) {
        match self.counts.iter_mut().find(|(known, _)| *known == reason) {
            Some((_, count)) => *count += 1,
            None => self.counts.push((reason, 1)),
        }
    }

    /// Releases one hold of the reason; releasing more holds than acquired is
    /// a caller bug and saturates at zero instead of panicking.
    pub fn release(&mut self, reason: SuppressionReason) {
        if let Some(position) = self.counts.iter().position(|(known, _)| *known == reason) {
            let (_, count) = &mut self.counts[position];
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.counts.remove(position);
            }
        }
    }

    pub fn is_suppressed(&self) -> bool {
        !self.counts.is_empty()
    }

    pub fn holds(&self, reason: SuppressionReason) -> u64 {
        self.counts
            .iter()
            .find(|(known, _)| *known == reason)
            .map(|(_, count)| *count)
            .unwrap_or(0)
    }
}

/// RAII suppression token from baseline 10.2. Dropping the token releases its
/// hold; the release path needs no caller cooperation.
#[derive(Debug)]
pub struct SuppressionToken {
    set: Arc<Mutex<SuppressionSet>>,
    reason: SuppressionReason,
}

impl SuppressionToken {
    pub fn reason(&self) -> SuppressionReason {
        self.reason
    }
}

impl Drop for SuppressionToken {
    fn drop(&mut self) {
        // The lock cannot meaningfully fail here: a poisoned lock means a
        // panic happened while holding it; releasing is still the best effort.
        if let Ok(mut set) = self.set.lock() {
            set.release(self.reason);
        }
    }
}

/// What the delayed blur re-check decided after re-reading the foreground
/// window (baseline 10.3 step 6-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlurRecheck {
    /// The foreground is QuickNest or one of its owned windows again; stay.
    Stay,
    /// The user really moved to an outside window; hide.
    Hide,
}

/// Outcome of the immediate blur observation, deciding whether a delayed
/// re-check is worth scheduling at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlurObservation {
    /// Auto-hide is disabled or the panel is hidden, suppressed or shutting down.
    Ignore,
    /// Panel is visible, unsuppressed and the foreground is ours.
    ForegroundOurs,
    /// Panel is visible, unsuppressed, and the foreground is outside: the
    /// caller must run the one-shot delayed re-check.
    StartDelayedCheck,
}

/// The visibility state machine. Pinned is managed as its own flag and, per
/// baseline 10.2, simultaneously registered as a Pinned suppression hold so
/// every hide path consults one mechanism.
#[derive(Debug)]
pub struct FocusStateMachine {
    state: PanelState,
    pinned: bool,
    auto_hide_on_blur_enabled: bool,
    /// Platform-fed hint for observe_blur; validation layers refresh it from
    /// GetForegroundWindow before consulting the machine.
    foreground_is_ours: bool,
    suppressions: Arc<Mutex<SuppressionSet>>,
}

impl Default for FocusStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusStateMachine {
    pub fn new() -> Self {
        Self {
            state: PanelState::Hidden,
            pinned: false,
            auto_hide_on_blur_enabled: true,
            foreground_is_ours: true,
            suppressions: Arc::new(Mutex::new(SuppressionSet::default())),
        }
    }

    pub fn state(&self) -> PanelState {
        self.state
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    /// Fixed state is managed independently of visibility (baseline 10.1);
    /// it is mirrored as a Pinned suppression hold (baseline 10.2/10.7).
    pub fn set_pinned(&mut self, pinned: bool) {
        if self.pinned == pinned {
            return;
        }
        self.pinned = pinned;
        let mut set = self.suppressions.lock().expect("suppression lock poisoned");
        if pinned {
            set.acquire(SuppressionReason::Pinned);
        } else {
            set.release(SuppressionReason::Pinned);
        }
    }

    pub fn set_auto_hide_on_blur(&mut self, enabled: bool) {
        self.auto_hide_on_blur_enabled = enabled;
    }

    pub fn is_suppressed(&self) -> bool {
        self.suppressions
            .lock()
            .expect("suppression lock poisoned")
            .is_suppressed()
    }

    pub fn holds(&self, reason: SuppressionReason) -> u64 {
        self.suppressions
            .lock()
            .expect("suppression lock poisoned")
            .holds(reason)
    }

    /// Acquires a suppression hold and hands out its RAII token.
    pub fn acquire(&self, reason: SuppressionReason) -> SuppressionToken {
        self.suppressions
            .lock()
            .expect("suppression lock poisoned")
            .acquire(reason);
        SuppressionToken {
            set: Arc::clone(&self.suppressions),
            reason,
        }
    }

    /// Immediate blur observation (baseline 10.3 steps 1-4). Returns whether
    /// a delayed re-check should be scheduled; it never hides by itself.
    /// The platform layer refreshes the foreground hint from
    /// GetForegroundWindow/GetWindowThreadProcessId before calling this.
    pub fn observe_blur(&self) -> BlurObservation {
        if !self.auto_hide_on_blur_enabled || self.state != PanelState::Visible {
            return BlurObservation::Ignore;
        }
        if self.is_suppressed() {
            return BlurObservation::Ignore;
        }
        if self.foreground_is_ours {
            BlurObservation::ForegroundOurs
        } else {
            BlurObservation::StartDelayedCheck
        }
    }

    /// Delayed re-check decision (baseline 10.3 steps 6-8): hide only when auto-hide
    /// is enabled, the foreground is still outside and no suppression appeared.
    pub fn evaluate_blur_recheck(&self, foreground_is_ours: bool) -> BlurRecheck {
        if !self.auto_hide_on_blur_enabled || self.state != PanelState::Visible {
            return BlurRecheck::Stay;
        }
        if self.is_suppressed() {
            return BlurRecheck::Stay;
        }
        if foreground_is_ours {
            BlurRecheck::Stay
        } else {
            BlurRecheck::Hide
        }
    }

    /// Hidden -> Showing -> Visible. From Visible this is a no-op returning
    /// the single-state path; other states refuse via ShuttingDown rules.
    pub fn show(&mut self) -> Result<Vec<PanelState>, StateError> {
        if self.state == PanelState::ShuttingDown {
            return Err(StateError::ShuttingDown);
        }
        if self.state == PanelState::Visible {
            return Ok(vec![PanelState::Visible]);
        }
        let mut path = Vec::with_capacity(3);
        if self.state != PanelState::Showing {
            path.push(PanelState::Showing);
        }
        self.state = PanelState::Showing;
        path.push(PanelState::Visible);
        self.state = PanelState::Visible;
        Ok(path)
    }

    /// Visible -> Hiding -> Hidden. Hidden is a no-op returning the path.
    pub fn hide(&mut self) -> Result<Vec<PanelState>, StateError> {
        if self.state == PanelState::ShuttingDown {
            return Err(StateError::ShuttingDown);
        }
        if self.state == PanelState::Hidden {
            return Ok(vec![PanelState::Hidden]);
        }
        let mut path = Vec::with_capacity(3);
        if self.state != PanelState::Hiding {
            path.push(PanelState::Hiding);
        }
        self.state = PanelState::Hiding;
        path.push(PanelState::Hidden);
        self.state = PanelState::Hidden;
        Ok(path)
    }

    /// Toggles visibility; the current state decides the direction.
    pub fn toggle(&mut self) -> Result<Vec<PanelState>, StateError> {
        if self.state == PanelState::Visible {
            self.hide()
        } else {
            self.show()
        }
    }

    /// Baseline 10.1: any state may enter ShuttingDown; further transitions
    /// are refused so late events cannot resurrect the panel.
    pub fn begin_shutdown(&mut self) -> PanelState {
        self.state = PanelState::ShuttingDown;
        self.state
    }

    pub fn set_foreground_hint(&mut self, is_ours: bool) {
        self.foreground_is_ours = is_ours;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_walks_showing_visible_and_is_a_noop_when_visible() {
        let mut machine = FocusStateMachine::new();
        assert_eq!(machine.state(), PanelState::Hidden);
        let path = machine.show().expect("show from Hidden");
        assert_eq!(path, vec![PanelState::Showing, PanelState::Visible]);
        assert_eq!(machine.state(), PanelState::Visible);
        let path = machine.show().expect("show while Visible");
        assert_eq!(path, vec![PanelState::Visible]);
    }

    #[test]
    fn hide_walks_hiding_hidden_and_is_a_noop_when_hidden() {
        let mut machine = FocusStateMachine::new();
        let path = machine.hide().expect("hide from Hidden");
        assert_eq!(path, vec![PanelState::Hidden]);
        machine.show().expect("show");
        let path = machine.hide().expect("hide from Visible");
        assert_eq!(path, vec![PanelState::Hiding, PanelState::Hidden]);
        assert_eq!(machine.state(), PanelState::Hidden);
    }

    #[test]
    fn toggle_follows_current_visibility() {
        let mut machine = FocusStateMachine::new();
        machine.toggle().expect("toggle to visible");
        assert_eq!(machine.state(), PanelState::Visible);
        machine.toggle().expect("toggle to hidden");
        assert_eq!(machine.state(), PanelState::Hidden);
    }

    #[test]
    fn shutting_down_refuses_transitions_from_any_state() {
        for start in [PanelState::Hidden, PanelState::Visible] {
            let mut machine = FocusStateMachine::new();
            if start == PanelState::Visible {
                machine.show().expect("setup visible");
            }
            assert_eq!(machine.begin_shutdown(), PanelState::ShuttingDown);
            assert!(matches!(machine.show(), Err(StateError::ShuttingDown)));
            assert!(matches!(machine.hide(), Err(StateError::ShuttingDown)));
            assert!(matches!(machine.toggle(), Err(StateError::ShuttingDown)));
        }
    }

    #[test]
    fn pinned_flag_survives_visibility_changes() {
        let mut machine = FocusStateMachine::new();
        machine.set_pinned(true);
        assert!(machine.is_pinned());
        machine.show().expect("show while pinned");
        assert!(machine.is_pinned());
        machine.hide().expect("hide while pinned");
        assert!(machine.is_pinned());
        machine.set_pinned(false);
        assert!(!machine.is_pinned());
    }

    #[test]
    fn pinned_registers_and_releases_one_pinned_hold() {
        let mut machine = FocusStateMachine::new();
        assert_eq!(machine.holds(SuppressionReason::Pinned), 0);
        machine.set_pinned(true);
        assert_eq!(machine.holds(SuppressionReason::Pinned), 1);
        assert!(machine.is_suppressed());
        machine.set_pinned(false);
        assert_eq!(machine.holds(SuppressionReason::Pinned), 0);
        assert!(!machine.is_suppressed());
    }

    #[test]
    fn token_drop_releases_its_hold_without_caller_help() {
        let machine = FocusStateMachine::new();
        {
            let token = machine.acquire(SuppressionReason::ContextMenuOpen);
            assert_eq!(token.reason(), SuppressionReason::ContextMenuOpen);
            assert_eq!(machine.holds(SuppressionReason::ContextMenuOpen), 1);
            assert!(machine.is_suppressed());
        }
        assert_eq!(machine.holds(SuppressionReason::ContextMenuOpen), 0);
        assert!(!machine.is_suppressed());
    }

    #[test]
    fn multiple_tokens_count_and_release_independently() {
        let machine = FocusStateMachine::new();
        let first = machine.acquire(SuppressionReason::ExternalDragHover);
        let second = machine.acquire(SuppressionReason::ExternalDragHover);
        let dialog = machine.acquire(SuppressionReason::ModalDialogOpen);
        assert_eq!(machine.holds(SuppressionReason::ExternalDragHover), 2);
        assert!(machine.is_suppressed());
        drop(first);
        assert_eq!(machine.holds(SuppressionReason::ExternalDragHover), 1);
        drop(second);
        assert_eq!(machine.holds(SuppressionReason::ExternalDragHover), 0);
        assert!(machine.is_suppressed(), "dialog token still holds");
        drop(dialog);
        assert!(!machine.is_suppressed());
    }

    #[test]
    fn external_drag_hover_blocks_blur_until_released() {
        let mut machine = FocusStateMachine::new();
        machine.show().expect("show");
        machine.set_foreground_hint(false);
        let token = machine.acquire(SuppressionReason::ExternalDragHover);
        assert_eq!(machine.observe_blur(), BlurObservation::Ignore);
        assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Stay);
        drop(token);
        assert_eq!(machine.observe_blur(), BlurObservation::StartDelayedCheck);
        assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Hide);
    }

    #[test]
    fn token_survives_machine_drop_and_still_releases() {
        let set = {
            let machine = FocusStateMachine::new();
            let token = machine.acquire(SuppressionReason::LaunchingElevated);
            let set = Arc::clone(&token.set);
            drop(token);
            drop(machine);
            assert_eq!(
                set.lock()
                    .expect("set")
                    .holds(SuppressionReason::LaunchingElevated),
                0
            );
            set
        };
        assert!(!set.lock().expect("set").is_suppressed());
    }

    #[test]
    fn release_beyond_acquired_saturates_at_zero() {
        let mut set = SuppressionSet::default();
        set.acquire(SuppressionReason::SystemDialogActive);
        set.release(SuppressionReason::SystemDialogActive);
        set.release(SuppressionReason::SystemDialogActive);
        assert!(!set.is_suppressed());
    }

    #[test]
    fn disabled_auto_hide_ignores_blur_and_rechecks() {
        let mut machine = FocusStateMachine::new();
        machine.show().expect("show");
        machine.set_auto_hide_on_blur(false);
        for foreground_is_ours in [false, true] {
            machine.set_foreground_hint(foreground_is_ours);
            assert_eq!(machine.observe_blur(), BlurObservation::Ignore);
            assert_eq!(
                machine.evaluate_blur_recheck(foreground_is_ours),
                BlurRecheck::Stay
            );
        }
        assert_eq!(machine.state(), PanelState::Visible);
        assert!(!machine.is_suppressed());
    }

    #[test]
    fn re_enabling_auto_hide_restores_blur_checks() {
        let mut machine = FocusStateMachine::new();
        machine.set_auto_hide_on_blur(false);
        machine.show().expect("show");
        machine.set_foreground_hint(false);
        assert_eq!(machine.observe_blur(), BlurObservation::Ignore);
        assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Stay);
        machine.set_auto_hide_on_blur(true);
        assert_eq!(machine.observe_blur(), BlurObservation::StartDelayedCheck);
        assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Hide);
        machine.set_foreground_hint(true);
        assert_eq!(machine.observe_blur(), BlurObservation::ForegroundOurs);
        assert_eq!(machine.evaluate_blur_recheck(true), BlurRecheck::Stay);
    }

    #[test]
    fn disabling_auto_hide_before_recheck_blocks_the_hide() {
        let mut machine = FocusStateMachine::new();
        machine.show().expect("show");
        machine.set_foreground_hint(false);
        assert_eq!(machine.observe_blur(), BlurObservation::StartDelayedCheck);
        machine.set_auto_hide_on_blur(false);
        assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Stay);
        assert_eq!(machine.observe_blur(), BlurObservation::Ignore);
        assert_eq!(machine.state(), PanelState::Visible);
    }

    #[test]
    fn disabled_auto_hide_preserves_explicit_visibility_and_shutdown() {
        let mut machine = FocusStateMachine::new();
        machine.set_auto_hide_on_blur(false);
        assert_eq!(
            machine.show().expect("show"),
            vec![PanelState::Showing, PanelState::Visible]
        );
        assert_eq!(
            machine.hide().expect("hide"),
            vec![PanelState::Hiding, PanelState::Hidden]
        );
        machine.toggle().expect("toggle to visible");
        assert_eq!(machine.state(), PanelState::Visible);
        machine.toggle().expect("toggle to hidden");
        assert_eq!(machine.state(), PanelState::Hidden);
        assert_eq!(machine.begin_shutdown(), PanelState::ShuttingDown);
        for enabled in [false, true] {
            machine.set_auto_hide_on_blur(enabled);
            assert_eq!(machine.observe_blur(), BlurObservation::Ignore);
            assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Stay);
            assert!(matches!(machine.show(), Err(StateError::ShuttingDown)));
            assert!(matches!(machine.hide(), Err(StateError::ShuttingDown)));
            assert!(matches!(machine.toggle(), Err(StateError::ShuttingDown)));
        }
    }

    #[test]
    fn auto_hide_setting_preserves_pinned_and_other_suppression() {
        let mut machine = FocusStateMachine::new();
        machine.show().expect("show");
        machine.set_foreground_hint(false);
        machine.set_pinned(true);
        let token = machine.acquire(SuppressionReason::ContextMenuOpen);
        for enabled in [false, true] {
            machine.set_auto_hide_on_blur(enabled);
            assert!(machine.is_pinned());
            assert_eq!(machine.holds(SuppressionReason::Pinned), 1);
            assert_eq!(machine.holds(SuppressionReason::ContextMenuOpen), 1);
            assert_eq!(machine.observe_blur(), BlurObservation::Ignore);
            assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Stay);
        }
        machine.set_pinned(false);
        assert_eq!(machine.observe_blur(), BlurObservation::Ignore);
        assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Stay);
        drop(token);
        assert!(!machine.is_suppressed());
        assert_eq!(machine.observe_blur(), BlurObservation::StartDelayedCheck);
        assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Hide);
    }

    #[test]
    fn blur_observation_respects_state_and_suppression() {
        let mut machine = FocusStateMachine::new();
        machine.set_foreground_hint(false);
        assert_eq!(machine.observe_blur(), BlurObservation::Ignore);
        machine.show().expect("show");
        assert_eq!(machine.observe_blur(), BlurObservation::StartDelayedCheck);
        machine.set_foreground_hint(true);
        assert_eq!(machine.observe_blur(), BlurObservation::ForegroundOurs);
        machine.set_foreground_hint(false);
        machine.set_pinned(true);
        assert_eq!(machine.observe_blur(), BlurObservation::Ignore);
        machine.set_pinned(false);
        let token = machine.acquire(SuppressionReason::ContextMenuOpen);
        assert_eq!(machine.observe_blur(), BlurObservation::Ignore);
        drop(token);
        assert_eq!(machine.observe_blur(), BlurObservation::StartDelayedCheck);
    }

    #[test]
    fn blur_recheck_hides_only_when_foreground_stays_outside() {
        let mut machine = FocusStateMachine::new();
        machine.show().expect("show");
        assert_eq!(
            machine.evaluate_blur_recheck(true),
            BlurRecheck::Stay,
            "user came back before the recheck; never hide on one blur"
        );
        assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Hide);
        machine.hide().expect("hide");
        assert_eq!(
            machine.evaluate_blur_recheck(false),
            BlurRecheck::Stay,
            "hidden panels never re-hide"
        );
    }

    #[test]
    fn suppression_gained_before_recheck_blocks_the_hide() {
        let mut machine = FocusStateMachine::new();
        machine.show().expect("show");
        let token = machine.acquire(SuppressionReason::ExternalDragHover);
        assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Stay);
        drop(token);
        assert_eq!(machine.evaluate_blur_recheck(false), BlurRecheck::Hide);
    }
}
