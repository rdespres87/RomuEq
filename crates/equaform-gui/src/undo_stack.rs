//! Bounded undo/redo history stack and gesture-coalescing controller for EQ parameters.

use std::collections::VecDeque;

use equaform_common::ParamId;

use crate::ab_compare::capture_eq_snapshot;
use crate::{GuiCommandBatch, ParamSnapshot};

/// Maximum number of undo history entries retained by default.
pub const DEFAULT_MAX_UNDO_HISTORY: usize = 64;

const VALUE_EPSILON: f32 = 1e-5;

/// A generic bounded undo/redo history stack.
#[derive(Clone, Debug)]
pub struct UndoStack<T> {
    undo: VecDeque<T>,
    redo: Vec<T>,
    max_history: usize,
}

impl<T> UndoStack<T> {
    /// Creates a new `UndoStack` with the given maximum history size (clamped to at least 1).
    pub fn new(max_history: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            max_history: max_history.max(1),
        }
    }

    /// Returns `true` if there is at least one entry available to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Returns `true` if there is at least one entry available to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Returns the number of entries in the undo history.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Returns the number of entries in the redo history.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Records a previous state before an edit, evicting the oldest entry if at capacity
    /// and clearing the redo stack.
    pub fn record(&mut self, previous_state: T) {
        if self.undo.len() >= self.max_history {
            self.undo.pop_front();
        }
        self.undo.push_back(previous_state);
        self.redo.clear();
    }

    /// Pops the most recent undo entry, pushing `current_state` onto the redo stack.
    pub fn undo(&mut self, current_state: T) -> Option<T> {
        let previous = self.undo.pop_back()?;
        self.redo.push(current_state);
        Some(previous)
    }

    /// Pops the most recent redo entry, pushing `current_state` onto the undo stack.
    pub fn redo(&mut self, current_state: T) -> Option<T> {
        let next = self.redo.pop()?;
        if self.undo.len() >= self.max_history {
            self.undo.pop_front();
        }
        self.undo.push_back(current_state);
        Some(next)
    }

    /// Clears all undo and redo history.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

impl<T> Default for UndoStack<T> {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_UNDO_HISTORY)
    }
}

/// Emits commands into `batch` for every parameter in `target` that differs from `current`
/// by more than `1e-5`, or is absent from `current`.
pub fn emit_snapshot_diff(
    target: &[(ParamId, f32)],
    current: &ParamSnapshot,
    batch: &mut GuiCommandBatch,
) {
    for &(id, target_val) in target {
        let needs_update = current
            .get(id)
            .map(|cur| (cur - target_val).abs() > VALUE_EPSILON)
            .unwrap_or(true);
        if needs_update {
            batch.push(id, target_val);
        }
    }
}

fn snapshots_approx_equal(a: &[(ParamId, f32)], b: &[(ParamId, f32)]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(&(id_a, val_a), &(id_b, val_b))| {
                id_a == id_b && (val_a - val_b).abs() <= VALUE_EPSILON
            })
}

/// Gesture-coalescing undo/redo controller for EQ workspace parameter snapshots.
#[derive(Clone, Debug, Default)]
pub struct EqUndoController {
    stack: UndoStack<Vec<(ParamId, f32)>>,
    gesture_baseline: Option<Vec<(ParamId, f32)>>,
}

impl EqUndoController {
    /// Returns `true` if an undo step is available.
    pub fn can_undo(&self) -> bool {
        self.stack.can_undo()
    }

    /// Returns `true` if a redo step is available.
    pub fn can_redo(&self) -> bool {
        self.stack.can_redo()
    }

    /// Returns the number of recorded undo steps.
    pub fn undo_len(&self) -> usize {
        self.stack.undo_len()
    }

    /// Returns the number of recorded redo steps.
    pub fn redo_len(&self) -> usize {
        self.stack.redo_len()
    }

    /// Clears all recorded history and any in-progress gesture baseline.
    pub fn clear(&mut self) {
        self.stack.clear();
        self.gesture_baseline = None;
    }

    /// Restores the previous EQ parameter snapshot, emitting only changed parameters into `batch`.
    pub fn undo(&mut self, snapshot: &ParamSnapshot, batch: &mut GuiCommandBatch) -> bool {
        self.gesture_baseline = None;
        let current = capture_eq_snapshot(snapshot);
        if let Some(previous) = self.stack.undo(current) {
            emit_snapshot_diff(&previous, snapshot, batch);
            true
        } else {
            false
        }
    }

    /// Re-applies the next undone EQ parameter snapshot, emitting only changed parameters into `batch`.
    pub fn redo(&mut self, snapshot: &ParamSnapshot, batch: &mut GuiCommandBatch) -> bool {
        self.gesture_baseline = None;
        let current = capture_eq_snapshot(snapshot);
        if let Some(next) = self.stack.redo(current) {
            emit_snapshot_diff(&next, snapshot, batch);
            true
        } else {
            false
        }
    }

    /// Tracks continuous pointer drags and discrete edits across UI frames, coalescing
    /// multi-frame drags into a single undo entry once the pointer is released.
    pub fn observe_frame(
        &mut self,
        pre_frame_snapshot: Vec<(ParamId, f32)>,
        changed_this_frame: bool,
        pointer_down: bool,
    ) {
        if changed_this_frame && pointer_down {
            self.gesture_baseline.get_or_insert(pre_frame_snapshot);
        } else if changed_this_frame && !pointer_down {
            let baseline = self.gesture_baseline.take().unwrap_or(pre_frame_snapshot);
            self.stack.record(baseline);
        } else if !changed_this_frame
            && !pointer_down
            && let Some(baseline) = self.gesture_baseline.take()
            && !snapshots_approx_equal(&baseline, &pre_frame_snapshot)
        {
            self.stack.record(baseline);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParamValue;
    use equaform_common::{band, global};

    fn snapshot_with_gain(gain_b0: f32) -> ParamSnapshot {
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
                param_id: ParamId(global::AUTO_GAIN_MODE),
                value: 0.0,
            },
            ParamValue {
                param_id: ParamId(100 + band::GAIN_DB),
                value: gain_b0,
            },
        ])
    }

    #[test]
    fn test_undo_redo_roundtrip() {
        let mut stack = UndoStack::new(10);
        stack.record(1);
        stack.record(2);
        assert_eq!(stack.undo_len(), 2);
        assert!(!stack.can_redo());

        assert_eq!(stack.undo(3), Some(2));
        assert_eq!(stack.undo(2), Some(1));
        assert_eq!(stack.undo(1), None);
        assert_eq!(stack.redo_len(), 2);

        assert_eq!(stack.redo(1), Some(2));
        assert_eq!(stack.redo(2), Some(3));
        assert_eq!(stack.redo(3), None);
    }

    #[test]
    fn test_record_clears_redo_stack() {
        let mut stack = UndoStack::new(10);
        stack.record(10);
        stack.record(20);
        assert_eq!(stack.undo(30), Some(20));
        assert!(stack.can_redo());

        stack.record(20);
        assert!(!stack.can_redo());
        assert_eq!(stack.redo_len(), 0);
    }

    #[test]
    fn test_bounded_capacity_evicts_oldest() {
        let mut stack = UndoStack::new(3);
        for v in 1..=5 {
            stack.record(v);
        }
        assert_eq!(stack.undo_len(), 3);
        assert_eq!(stack.undo(6), Some(5));
        assert_eq!(stack.undo(5), Some(4));
        assert_eq!(stack.undo(4), Some(3));
        assert_eq!(stack.undo(3), None);
    }

    #[test]
    fn test_undo_redo_empty_returns_none() {
        let mut stack: UndoStack<i32> = UndoStack::default();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo(0), None);
        assert_eq!(stack.redo(0), None);
    }

    #[test]
    fn test_drag_gesture_coalesces_into_single_undo_entry() {
        let mut ctrl = EqUndoController::default();
        let s0 = snapshot_with_gain(0.0);
        let s1 = snapshot_with_gain(2.0);
        let s2 = snapshot_with_gain(4.5);
        let s3 = snapshot_with_gain(6.0);

        ctrl.observe_frame(capture_eq_snapshot(&s0), true, true);
        ctrl.observe_frame(capture_eq_snapshot(&s1), true, true);
        ctrl.observe_frame(capture_eq_snapshot(&s2), true, true);
        assert!(!ctrl.can_undo(), "should not record while pointer is down");

        // Release pointer at final gain 6.0
        ctrl.observe_frame(capture_eq_snapshot(&s3), false, false);
        assert_eq!(ctrl.undo_len(), 1);

        let mut batch = GuiCommandBatch::new();
        assert!(ctrl.undo(&s3, &mut batch));
        assert_eq!(batch.commands().len(), 1);
        assert_eq!(batch.commands()[0].param_id, ParamId(100 + band::GAIN_DB));
        assert!((batch.commands()[0].value - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_drag_returning_to_origin_records_nothing() {
        let mut ctrl = EqUndoController::default();
        let s0 = snapshot_with_gain(0.0);
        let s1 = snapshot_with_gain(3.0);

        ctrl.observe_frame(capture_eq_snapshot(&s0), true, true);
        ctrl.observe_frame(capture_eq_snapshot(&s1), true, true);
        // Dragged back to 0.0 and released
        ctrl.observe_frame(capture_eq_snapshot(&s0), false, false);
        assert!(!ctrl.can_undo());
    }

    #[test]
    fn test_discrete_edit_records_immediately_and_restores_diff() {
        let mut ctrl = EqUndoController::default();
        let s0 = snapshot_with_gain(0.0);
        let s1 = snapshot_with_gain(-3.5);

        ctrl.observe_frame(capture_eq_snapshot(&s0), true, false);
        assert_eq!(ctrl.undo_len(), 1);

        let mut undo_batch = GuiCommandBatch::new();
        assert!(ctrl.undo(&s1, &mut undo_batch));
        assert_eq!(undo_batch.commands().len(), 1);
        assert!((undo_batch.commands()[0].value - 0.0).abs() < 1e-5);

        let mut redo_batch = GuiCommandBatch::new();
        assert!(ctrl.redo(&s0, &mut redo_batch));
        assert_eq!(redo_batch.commands().len(), 1);
        assert!((redo_batch.commands()[0].value - (-3.5)).abs() < 1e-5);
    }
}
