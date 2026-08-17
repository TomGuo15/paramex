use paramex_gui::state::EditBuffers;
use paramex_gui::workspaces::transfer::state::{CoxUi, GeometryUi, COX_ESTIMATE_PENDING_LABEL};

#[test]
fn buffer_inits_to_current_then_take_clears() {
    let mut b = EditBuffers::default();
    // Absent -> initialized to the committed value's string.
    assert_eq!(b.buffer("geom:a:w", "1500"), "1500");
    // Present -> returned as-is (user-edited value survives the redraw).
    b.buffer("geom:a:w", "1500").push('0'); // simulate typing -> "15000"
    assert_eq!(b.buffer("geom:a:w", "1500"), "15000");
    // Commit consumes the buffer.
    assert_eq!(b.take("geom:a:w"), Some("15000".to_string()));
    assert_eq!(b.take("geom:a:w"), None);
    // After commit it re-syncs to the new committed value.
    assert_eq!(b.buffer("geom:a:w", "15000"), "15000");
}

#[test]
fn forget_drops_stale_buffer_for_resync() {
    let mut b = EditBuffers::default();
    b.buffer("k", "10").push('9'); // "109" pending
    b.forget("k");
    // Dropped -> re-init to current committed value on next access.
    assert_eq!(b.buffer("k", "42"), "42");
}

#[test]
fn geometry_and_cox_defaults_match_python() {
    let g = GeometryUi::default();
    assert_eq!(g.global_w(), "1500"); // geometry_panel.py:85 default 1500.0
    assert_eq!(g.global_l(), "50"); // geometry_panel.py:90 default 50.0

    let c = CoxUi::default();
    // Initial first layer (3.9, 300.0) — cox_layer_panel.py:47.
    assert_eq!(c.layers().len(), 1);
    assert_eq!(c.layers()[0].eps_text(), "3.9");
    assert_eq!(c.layers()[0].th_text(), "300");
    // Estimate label uses `<sub>` markup rendered via `richtext` (the old Unicode
    // subscripts C\u{2092}\u{2093} rendered as tofu boxes — no font covers them).
    assert_eq!(c.estimate_label(), COX_ESTIMATE_PENDING_LABEL);
    assert_eq!(c.estimate_value(), None);
}

/// take_on_commit's three branches: commit on lost_focus, drop while unfocused,
/// keep while focused.
#[test]
fn take_on_commit_branches() {
    use paramex_gui::state::EditBuffers;
    let mut edits = EditBuffers::default();

    // lost_focus -> the buffered text comes back exactly once.
    edits.buffer("k", "seed").push('!');
    assert_eq!(
        edits.take_on_commit("k", true, false).as_deref(),
        Some("seed!")
    );
    assert_eq!(edits.take_on_commit("k", true, false), None); // consumed

    // unfocused (no commit) -> the buffer is forgotten (re-syncs next frame).
    edits.buffer("k", "seed");
    assert_eq!(edits.take_on_commit("k", false, false), None);
    assert_eq!(edits.buffer("k", "fresh"), "fresh"); // re-seeded, old text gone

    // still focused -> the buffer survives untouched.
    edits.buffer("j", "typing");
    assert_eq!(edits.take_on_commit("j", false, true), None);
    assert_eq!(edits.buffer("j", "ignored"), "typing");
}
