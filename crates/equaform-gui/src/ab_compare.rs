//! A/B Compare snapshot state for instant comparison of two EQ configurations.

use crate::{GuiCommandBatch, ParamSnapshot};
use equaform_common::{MAX_BANDS, ParamId, band, global};

/// Active A/B comparison slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AbSlot {
    #[default]
    A,
    B,
}

impl AbSlot {
    /// Returns the opposite slot.
    pub const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }

    /// Compact toolbar label for copying the active slot into the inactive slot.
    pub const fn copy_label(self) -> &'static str {
        match self {
            Self::A => "A>B",
            Self::B => "B>A",
        }
    }
}

/// Holds two independent EQ parameter snapshots (`A` and `B`) and the active slot.
#[derive(Clone, Debug, Default)]
pub struct AbCompareState {
    active: AbSlot,
    slot_a: Option<Vec<(ParamId, f32)>>,
    slot_b: Option<Vec<(ParamId, f32)>>,
}

/// Captures all persistent EQ parameters from `snapshot`, excluding monitor-only
/// solo state and read-only auto-gain telemetry.
pub fn capture_eq_snapshot(snapshot: &ParamSnapshot) -> Vec<(ParamId, f32)> {
    let mut out = Vec::with_capacity(4 + MAX_BANDS * 7);

    for raw_id in [
        global::BYPASS,
        global::OUTPUT_GAIN_DB,
        global::AUTO_GAIN,
        global::AUTO_GAIN_MODE,
    ] {
        let id = ParamId(raw_id);
        if let Some(val) = snapshot.get(id) {
            out.push((id, val));
        }
    }

    for i in 0..MAX_BANDS {
        let base = 100 + i as u32 * 100;
        for off in [
            band::ENABLED,
            band::FILTER_TYPE,
            band::FREQUENCY_HZ,
            band::GAIN_DB,
            band::Q,
            band::SLOPE,
            band::ROUTING,
        ] {
            let id = ParamId(base + off);
            if let Some(val) = snapshot.get(id) {
                out.push((id, val));
            }
        }
    }

    out
}

impl AbCompareState {
    /// Returns the currently active comparison slot (`A` or `B`).
    pub const fn active(&self) -> AbSlot {
        self.active
    }

    /// Lazily initializes both slots from the first available snapshot.
    pub fn ensure_initialized(&mut self, snapshot: &ParamSnapshot) {
        if self.slot_a.is_none() || self.slot_b.is_none() {
            let captured = capture_eq_snapshot(snapshot);
            if self.slot_a.is_none() {
                self.slot_a = Some(captured.clone());
            }
            if self.slot_b.is_none() {
                self.slot_b = Some(captured);
            }
        }
    }

    /// Switches to `target` slot, saving current live parameters into the outgoing
    /// slot and emitting only changed parameters into `batch`.
    pub fn switch_to(
        &mut self,
        target: AbSlot,
        snapshot: &ParamSnapshot,
        batch: &mut GuiCommandBatch,
    ) {
        self.ensure_initialized(snapshot);
        if target == self.active {
            return;
        }

        let outgoing = capture_eq_snapshot(snapshot);
        match self.active {
            AbSlot::A => self.slot_a = Some(outgoing),
            AbSlot::B => self.slot_b = Some(outgoing),
        }

        self.active = target;
        let target_params = match self.active {
            AbSlot::A => self.slot_a.as_deref(),
            AbSlot::B => self.slot_b.as_deref(),
        };

        if let Some(params) = target_params {
            for &(id, target_val) in params {
                let needs_update = snapshot
                    .get(id)
                    .map(|cur| (cur - target_val).abs() > 1e-6)
                    .unwrap_or(true);
                if needs_update {
                    batch.push(id, target_val);
                }
            }
        }
    }

    /// Copies the current active live parameters into the inactive slot.
    pub fn copy_active_to_other(&mut self, snapshot: &ParamSnapshot) {
        let captured = capture_eq_snapshot(snapshot);
        self.slot_a = Some(captured.clone());
        self.slot_b = Some(captured);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParamValue;

    fn make_snapshot(gain_b0: f32, solo_b0: f32) -> ParamSnapshot {
        ParamSnapshot::new(vec![
            ParamValue {
                param_id: ParamId(global::BYPASS),
                value: 0.0,
            },
            ParamValue {
                param_id: ParamId(global::OUTPUT_GAIN_DB),
                value: 0.0,
            },
            ParamValue {
                param_id: ParamId(global::AUTO_GAIN),
                value: 0.0,
            },
            ParamValue {
                param_id: ParamId(100 + band::GAIN_DB),
                value: gain_b0,
            },
            ParamValue {
                param_id: ParamId(100 + band::SOLO),
                value: solo_b0,
            },
        ])
    }

    #[test]
    fn test_initial_state_is_slot_a() {
        let state = AbCompareState::default();
        assert_eq!(state.active(), AbSlot::A);
        assert_eq!(state.active().other(), AbSlot::B);
        assert_eq!(state.active().copy_label(), "A>B");
    }

    #[test]
    fn test_switch_roundtrip_emits_only_diffs() {
        let mut state = AbCompareState::default();
        let snap_initial = make_snapshot(0.0, 0.0);
        state.ensure_initialized(&snap_initial);

        // User edits slot A to +6 dB, then switches to B (still 0 dB).
        let snap_a_edited = make_snapshot(6.0, 0.0);
        let mut batch = GuiCommandBatch::new();
        state.switch_to(AbSlot::B, &snap_a_edited, &mut batch);
        assert_eq!(state.active(), AbSlot::B);
        assert_eq!(batch.commands().len(), 1);
        assert_eq!(batch.commands()[0].param_id, ParamId(100 + band::GAIN_DB));
        assert!((batch.commands()[0].value - 0.0).abs() < 1e-6);

        // Switch back to A -> restores +6 dB only.
        let mut batch_back = GuiCommandBatch::new();
        state.switch_to(AbSlot::A, &snap_initial, &mut batch_back);
        assert_eq!(state.active(), AbSlot::A);
        assert_eq!(batch_back.commands().len(), 1);
        assert!((batch_back.commands()[0].value - 6.0).abs() < 1e-6);
    }

    #[test]
    fn test_copy_active_to_other_overwrites_inactive_slot() {
        let mut state = AbCompareState::default();
        state.ensure_initialized(&make_snapshot(0.0, 0.0));

        let snap_a = make_snapshot(9.0, 0.0);
        state.copy_active_to_other(&snap_a);

        // Switching to B now emits 0 diffs because B was updated to match A (+9 dB).
        let mut batch = GuiCommandBatch::new();
        state.switch_to(AbSlot::B, &snap_a, &mut batch);
        assert_eq!(state.active(), AbSlot::B);
        assert!(batch.commands().is_empty());
    }

    #[test]
    fn test_solo_is_excluded_from_ab_snapshots() {
        let captured = capture_eq_snapshot(&make_snapshot(3.0, 1.0));
        assert!(
            captured
                .iter()
                .all(|&(id, _)| id != ParamId(100 + band::SOLO))
        );
    }
}
