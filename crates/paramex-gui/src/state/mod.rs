//! Transient (non-committed) UI state grouped by concern. The committed
//! state is `core::Session`, owned by `ParamExApp`.

mod chrome;

pub use chrome::EasterEgg;

use std::collections::HashMap;

/// Focus-tracked numeric-input buffers keyed by a stable string. A field
/// commits through the shared input recipe; while unfocused its buffer is dropped
/// so it re-syncs to the committed value. Per-keystroke edits never escape the buffer.
#[derive(Debug, Default)]
pub struct EditBuffers {
    map: HashMap<String, String>,
}

impl EditBuffers {
    /// The editable buffer for `key`, initialized to `current` when absent.
    pub fn buffer(&mut self, key: &str, current: &str) -> &mut String {
        self.map
            .entry(key.to_string())
            .or_insert_with(|| current.to_string())
    }

    /// Remove and return the buffer (call on commit / `lost_focus`).
    pub fn take(&mut self, key: &str) -> Option<String> {
        self.map.remove(key)
    }

    /// Drop the buffer so it re-syncs to the committed value next frame.
    pub fn forget(&mut self, key: &str) {
        self.map.remove(key);
    }

    /// Drop every buffer whose key starts with `prefix`. Used when a sibling action mutates
    /// the committed state of a whole group of fields underneath their per-field buffers —
    /// e.g. a global "Apply W/L to All Files" changes every `geom:{id}:w|l` value, so a field
    /// that was focused at click time must not commit its now-stale buffer over the apply.
    pub fn forget_prefix(&mut self, prefix: &str) {
        self.map.retain(|key, _| !key.starts_with(prefix));
    }

    /// The whole commit-on-`lost_focus` ritual in one place: returns the text
    /// to commit on the `lost_focus` frame; otherwise drops the buffer while
    /// unfocused (re-sync) and returns `None`. Takes the two focus booleans
    /// rather than an `egui::Response` so this module stays egui-free (and the
    /// branches stay unit-testable).
    pub fn take_on_commit(
        &mut self,
        key: &str,
        lost_focus: bool,
        has_focus: bool,
    ) -> Option<String> {
        if lost_focus {
            self.take(key)
        } else {
            if !has_focus {
                self.forget(key);
            }
            None
        }
    }
}

/// Which workspace the banner toggle is showing. `Transfer` is the transfer-curve
/// extraction page; TLM is always labelled by its acronym. Keep the user-facing
/// `Transfer` label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Workspace {
    #[default]
    Transfer,
    Tlm,
    /// Compact-model fitting. Tab label "Model Fit"; keep the `Model` variant
    /// aligned with the persisted workspace state. The core seam is
    /// `paramex_core::modelfit`.
    Model,
}

impl Workspace {
    /// 0 = Transfer, 1 = Tlm, 2 = Model - the segmented-toggle index.
    pub fn index(self) -> usize {
        match self {
            Workspace::Transfer => 0,
            Workspace::Tlm => 1,
            Workspace::Model => 2,
        }
    }

    /// Inverse of [`index`]; anything outside 1..=2 is Transfer.
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Workspace::Tlm,
            2 => Workspace::Model,
            _ => Workspace::Transfer,
        }
    }
}
