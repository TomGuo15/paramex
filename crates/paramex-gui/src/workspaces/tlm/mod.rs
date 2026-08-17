mod ingest;
pub mod layout;
pub mod page;
pub mod panels;
pub mod state;

pub use page::show;

use crate::io_tasks::IoQueue;

/// TLM's complete runtime aggregate.
#[derive(Default)]
pub struct TlmWorkspace {
    pub(crate) state: state::TlmState,
    pub(crate) io: IoQueue<ingest::Msg>,
}

impl TlmWorkspace {
    pub fn from_state(state: state::TlmState) -> Self {
        Self {
            state,
            io: IoQueue::default(),
        }
    }

    pub fn state(&self) -> &state::TlmState {
        &self.state
    }

    pub(crate) fn drain_ingest(&mut self, toasts: &mut egui_notify::Toasts) {
        ingest::drain(self, toasts);
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.io.is_idle()
    }
}
