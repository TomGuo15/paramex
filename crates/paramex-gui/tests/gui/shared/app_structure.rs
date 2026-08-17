use paramex_core::transfer::Session;
use paramex_gui::app::ParamExApp;
use paramex_gui::state::Workspace;

#[test]
fn app_exposes_tlm_state_accessors() {
    let mut app = ParamExApp::from_session(Session::new());

    assert!(!app.tlm().has_dataset());

    app.set_active_workspace(Workspace::Tlm);
    app.tlm_mut().set_fallback_vd(0.25).expect("valid fallback");
    assert_eq!(app.tlm().fallback_vd(), 0.25);

    app.set_tlm_state(Default::default());
    assert_eq!(app.tlm().fallback_vd(), -0.5);
}
