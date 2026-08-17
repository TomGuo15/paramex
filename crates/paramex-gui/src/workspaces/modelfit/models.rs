//! Compact-model registry backing the Model Fit model selector.
//! Two models ship today: AOSTFT/UMEM and Level 62-derived (LTPS, stabilized).

use paramex_core::modelfit::FitModel;

/// One supported entry in the model menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModelEntry {
    pub name: &'static str,
    pub fit_model: FitModel,
}

/// Index of the AOSTFT / UMEM entry in [`FIT_MODELS`] — the default model.
pub const AOSTFT_INDEX: usize = 0;

/// Index of the Level 62-derived (LTPS / poly-Si, stabilized) entry in [`FIT_MODELS`].
pub const LEVEL62_INDEX: usize = 1;

/// The model menu, in display order. Index 0 (AOSTFT) is the default; index 1
/// (Level 62-derived / LTPS, stabilized) is the other supported model.
pub const FIT_MODELS: &[ModelEntry] = &[
    ModelEntry {
        name: "AOSTFT / UMEM",
        fit_model: FitModel::Aostft,
    },
    ModelEntry {
        name: "Level 62 / LTPS",
        fit_model: FitModel::Level62,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level62_is_the_second_supported_model() {
        assert_eq!(FIT_MODELS.len(), 2);
        assert_eq!(LEVEL62_INDEX, 1);
        assert_eq!(FIT_MODELS[LEVEL62_INDEX].name, "Level 62 / LTPS");
        assert_eq!(FIT_MODELS[AOSTFT_INDEX].fit_model, FitModel::Aostft);
        assert_eq!(FIT_MODELS[LEVEL62_INDEX].fit_model, FitModel::Level62);
    }
}
