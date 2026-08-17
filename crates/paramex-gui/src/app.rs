//! `ParamExApp` — the `eframe::App`. Owns committed state (`core::Session`) plus
//! transient sibling structs. Page modules render from it each frame.

mod brand_bar;
mod ingest;
mod page_dispatch;
mod shell;

use eframe::egui;
use egui_notify::{Anchor, Toasts};
use paramex_core::transfer::Session;

use crate::state::{EasterEgg, EditBuffers, Workspace};
use crate::workspaces::modelfit::state::ModelFitState;
use crate::workspaces::modelfit::ModelFitWorkspace;
use crate::workspaces::tlm::state::TlmState;
use crate::workspaces::tlm::TlmWorkspace;
use crate::workspaces::transfer::TransferWorkspace;

pub struct ParamExApp {
    pub(crate) transfer: TransferWorkspace,
    pub(crate) tlm: TlmWorkspace,
    pub(crate) modelfit: ModelFitWorkspace,
    pub(crate) edits: EditBuffers,
    pub(crate) egg: EasterEgg,
    pub(crate) toasts: Toasts,
    pub(crate) active_workspace: Workspace,
    pub(crate) show_help: bool,
    pub(crate) help_workspace: Workspace,
    pub(crate) help_model: usize,
}

impl ParamExApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::install(&cc.egui_ctx);
        Self::from_session(Session::new())
    }
}

impl ParamExApp {
    /// Build an app around a pre-loaded `Session` without a `CreationContext`,
    /// for headless render/snapshot tests. The caller installs the theme on the
    /// egui `Context` itself (e.g. `crate::theme::install(ui.ctx())`).
    pub fn from_session(session: Session) -> Self {
        ParamExApp {
            transfer: TransferWorkspace::from_session(session),
            tlm: TlmWorkspace::default(),
            modelfit: ModelFitWorkspace::default(),
            edits: EditBuffers::default(),
            egg: EasterEgg::default(),
            toasts: Toasts::default().with_anchor(Anchor::TopRight),
            active_workspace: Workspace::default(),
            show_help: false,
            help_workspace: Workspace::default(),
            help_model: 0,
        }
    }

    pub fn transfer_mut(&mut self) -> &mut TransferWorkspace {
        &mut self.transfer
    }

    pub fn tlm(&self) -> &TlmState {
        &self.tlm.state
    }

    pub fn tlm_mut(&mut self) -> &mut TlmState {
        &mut self.tlm.state
    }

    pub fn set_tlm_state(&mut self, tlm: TlmState) {
        self.tlm.state = tlm;
    }

    pub fn set_active_workspace(&mut self, workspace: Workspace) {
        self.active_workspace = workspace;
    }

    pub fn modelfit(&self) -> &ModelFitState {
        &self.modelfit.state
    }

    pub fn modelfit_mut(&mut self) -> &mut ModelFitState {
        &mut self.modelfit.state
    }

    pub fn modelfit_workspace(&self) -> &ModelFitWorkspace {
        &self.modelfit
    }

    pub fn modelfit_workspace_mut(&mut self) -> &mut ModelFitWorkspace {
        &mut self.modelfit
    }
}

impl eframe::App for ParamExApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }
}
