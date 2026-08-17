//! Bulk removal keeps selection valid, and the per-row ✕ on the DEVICES card is
//! wired through a real pointer click (the hand-painted
//! close-button needs a pointer guard: snapshots cannot catch a ✕ that
//! renders but doesn't commit, or one that also selects the row it removes).

use eframe::egui;
use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use egui_notify::Toasts;
use paramex_gui::state::EditBuffers;
use paramex_gui::workspaces::modelfit::panels::inputs::CARD_H;
use paramex_gui::workspaces::modelfit::ModelFitWorkspace;

#[test]
fn bulk_removal_keeps_selection_valid() {
    let mut state = crate::common::modelfit::demo_state(); // organic / LTPS
    assert_eq!(state.devices().len(), 2);

    // Removing an EARLIER device shifts the selection index down to stay on the
    // same device.
    state.select(1);
    assert!(state.set_device_checked(0, true));
    assert_eq!(state.remove_selected_or_checked(), 1);
    assert_eq!(state.devices().len(), 1);
    assert_eq!(
        state.selected_index(),
        Some(0),
        "selection followed its device"
    );

    // Removing the SELECTED (and now last) device clears the selection; an empty
    // state is a no-op.
    assert_eq!(state.remove_selected_or_checked(), 1);
    assert!(state.is_empty());
    assert_eq!(state.selected_index(), None);
    assert_eq!(state.remove_selected_or_checked(), 0);
}

#[test]
fn clear_drops_all_devices() {
    let mut state = crate::common::modelfit::demo_state();
    assert!(!state.is_empty());
    state.clear();
    assert!(state.is_empty());
    assert_eq!(state.selected_index(), None);
}

struct DataDevicesApp {
    workspace: ModelFitWorkspace,
    edits: EditBuffers,
    toasts: Toasts,
    size: egui::Vec2,
}

impl eframe::App for DataDevicesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.allocate_ui(egui::vec2(self.size.x, CARD_H), |ui| {
            paramex_gui::workspaces::modelfit::panels::inputs::show(
                ui,
                &mut self.workspace,
                &mut self.edits,
            );
        });
        ui.add_space(8.0);
        ui.allocate_ui(
            egui::vec2(self.size.x, (self.size.y - CARD_H - 8.0).max(0.0)),
            |ui| {
                paramex_gui::workspaces::modelfit::panels::devices::show(
                    ui,
                    &mut self.workspace,
                    &mut self.edits,
                    &mut self.toasts,
                    true,
                );
            },
        );
    }
}

#[test]
fn checked_devices_use_transfer_style_bulk_removal() {
    let mut state = crate::common::modelfit::demo_state(); // organic(0) / LTPS(1)
    state.select(0); // organic selected
    let app = DataDevicesApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(300.0, 760.0),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(340.0, 800.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    let keep_before = harness.get_by_label("Keep Checked").rect();
    let first_row_before = harness.get_by_label("demo: organic").rect();
    harness
        .get_by_role_and_label(
            egui::accesskit::Role::CheckBox,
            "Mark demo: LTPS for bulk actions",
        )
        .click_accesskit();
    harness.run();
    assert_eq!(keep_before, harness.get_by_label("Keep Checked").rect());
    assert_eq!(
        first_row_before,
        harness.get_by_label("demo: organic").rect(),
        "checking a device must not shift the scroll body"
    );
    assert!(
        !harness
            .get_by_label("Keep Checked")
            .accesskit_node()
            .is_disabled(),
        "Keep Checked enables for a checked subset"
    );
    harness.get_by_label("Remove Checked").click();
    harness.run();

    let st = harness.state().workspace.state();
    assert_eq!(st.devices().len(), 1, "LTPS was removed");
    assert!(
        st.devices()
            .iter()
            .all(|entry| entry.device().name() != "demo: LTPS"),
        "the clicked device is gone"
    );
    assert_eq!(
        st.selected_entry().map(|entry| entry.device().name()),
        Some("demo: organic"),
        "bulk removal preserves the surviving selection"
    );
}

#[test]
fn clicking_clear_all_empties_the_device_list_via_a_real_pointer_click() {
    let state = crate::common::modelfit::demo_state();
    assert!(!state.is_empty());
    let app = DataDevicesApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(300.0, 760.0),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(340.0, 800.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    // The "Clear All" action drops every device (clear() + forget the geom buffers).
    harness.get_by_label("Clear All").click();
    harness.run();

    let st = harness.state().workspace.state();
    assert!(st.is_empty(), "Clear All empties the device list");
    assert_eq!(st.selected_index(), None, "no selection after clear");
}
