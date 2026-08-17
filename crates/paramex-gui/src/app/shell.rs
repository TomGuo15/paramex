//! Shared app shell chrome and workspace page dispatch.

use eframe::egui;

use super::brand_bar;
use super::ParamExApp;
use crate::layout::{self, ShellRects};
use crate::ui_kit;
use crate::workspaces::modelfit::models::{AOSTFT_INDEX, FIT_MODELS, LEVEL62_INDEX};

impl ParamExApp {
    /// Render the whole window layout into `ui`. Shared by the eframe entry
    /// point (`App::ui`) and the headless snapshot test, so the test exercises
    /// the real panel layout rather than a reconstruction.
    pub fn render(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.drain_ingest(&ctx);

        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, crate::theme::tokens().bg);
        let shell = ShellRects::from_content(ui.max_rect());
        // Paint the ink banner across the whole top rect first: the brand-bar
        // content alone does not fill the width, so a content-sized Frame would
        // leave the right side unpainted. Then render the content on top.
        ui.painter()
            .rect_filled(shell.top, 0.0, crate::theme::tokens().ink);
        let help_was_open = self.show_help;
        layout::show_in_rect(ui, "brand_bar_rect", shell.top, |ui| {
            brand_bar::show(
                ui,
                &mut self.egg,
                &mut self.active_workspace,
                &mut self.show_help,
            );
        });
        let help_just_opened = self.show_help && !help_was_open;
        if help_just_opened {
            self.help_workspace = self.active_workspace;
            self.help_model = self.modelfit.state.selected_model();
        }

        super::page_dispatch::show_active_workspace(ui, &ctx, &shell, self);
        show_help_window(
            &ctx,
            &mut self.show_help,
            &mut self.help_workspace,
            &mut self.help_model,
            help_just_opened,
        );

        self.toasts.show(&ctx);
    }
}

const GUIDE_TABS: [&str; 3] = ["Transfer", "TLM", "Model Fit"];
const GUIDE_TAB_LABELS: [&str; 3] = ["Transfer guide", "TLM guide", "Model Fit guide"];
const MODEL_TABS: [&str; 2] = [
    FIT_MODELS[AOSTFT_INDEX].name,
    FIT_MODELS[LEVEL62_INDEX].name,
];
const MODEL_TAB_LABELS: [&str; 2] = MODEL_TABS;
const GUIDE_WIDTH: f32 = 900.0;
const GUIDE_MAX_BODY_HEIGHT: f32 = 620.0;
const GUIDE_TEXT_WIDTH: f32 = 660.0;
const GUIDE_CONTRACT_WIDTH: f32 = 720.0;
const GUIDE_CONTRACT_KEY_WIDTH: f32 = 64.0;

fn show_help_window(
    ctx: &egui::Context,
    open: &mut bool,
    page: &mut crate::state::Workspace,
    model: &mut usize,
    just_opened: bool,
) {
    if !*open {
        return;
    }
    egui_extras::install_image_loaders(ctx);
    let mut close_clicked = false;
    let body_height = (ctx.viewport_rect().height() - 120.0).clamp(360.0, GUIDE_MAX_BODY_HEIGHT);
    let response = egui::Modal::new(egui::Id::new("technical_guide"))
        .frame(ui_kit::card_frame())
        .backdrop_color(crate::theme::token_alpha(crate::theme::tokens().ink, 128))
        .show(ctx, |ui| {
            ui.set_width(GUIDE_WIDTH);
            ui.set_height(body_height + 30.0);
            let ((next_page, next_model), close) = ui_kit::header_nav_action_row(
                ui,
                "TECHNICAL GUIDE",
                |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let next_page = ui_kit::segmented_with_accessibility_labels(
                        ui,
                        &GUIDE_TABS,
                        &GUIDE_TAB_LABELS,
                        page.index(),
                        ui_kit::SegStyle::Card,
                        None,
                    );
                    let next_model = if *page == crate::state::Workspace::Model {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                        ui_kit::segmented_with_accessibility_labels(
                            ui,
                            &MODEL_TABS,
                            &MODEL_TAB_LABELS,
                            (*model).min(MODEL_TABS.len() - 1),
                            ui_kit::SegStyle::Card,
                            None,
                        )
                    } else {
                        None
                    };
                    (next_page, next_model)
                },
                |ui| ui_kit::close_button(ui, "Close guide").clicked(),
            );
            if let Some(index) = next_page {
                *page = crate::state::Workspace::from_index(index);
            }
            close_clicked = close;
            if let Some(index) = next_model {
                *model = index;
            }

            egui::ScrollArea::vertical()
                .id_salt(("technical_guide_body", page.index(), *model))
                .auto_shrink([false, true])
                .max_height(body_height)
                .min_scrolled_height(0.0)
                .scroll_bar_visibility(
                    egui::containers::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                )
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 7.0;
                    match *page {
                        crate::state::Workspace::Transfer => transfer_guide(ui),
                        crate::state::Workspace::Tlm => tlm_guide(ui),
                        crate::state::Workspace::Model => model_fit_guide(ui, *model),
                    }
                });
        });
    if close_clicked || (!just_opened && response.should_close()) {
        *open = false;
    }
}

fn transfer_guide(ui: &mut egui::Ui) {
    ui_kit::section_header(ui, "INPUT", None);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        guide_contract_row(ui, "Files", ".csv · .tsv · .txt · .xlsx · .xls");
        guide_contract_row(ui, "Columns", "V<sub>G</sub> · I<sub>D</sub>");
        guide_contract_row(
            ui,
            "Points",
            "At least 12 measured points across the gate sweep.",
        );
    });

    ui.add_space(14.0);
    ui_kit::section_header(ui, "FIT MATHEMATICS", None);
    guide_term(ui, "Threshold + saturation mobility");
    guide_math(
        ui,
        "Square root of absolute drain current equals m V G plus b; V T H equals minus b over m; mu sat equals two m squared over C ox times W over L",
        egui::include_image!("../../assets/math/transfer-threshold.svg"),
    );
    guide_text(ui, "Manual fit range: at least 5 points.");
    guide_term(ui, "Subthreshold swing");
    guide_math(
        ui,
        "Log base ten absolute drain current equals s V G plus c; subthreshold swing equals absolute one thousand over s millivolts per decade",
        egui::include_image!("../../assets/math/transfer-subthreshold.svg"),
    );
    guide_term(ui, "On/off + round-trip hysteresis");
    guide_math(
        ui,
        "I on equals maximum absolute I D; I off equals minimum positive absolute I D; on over off equals I on divided by I off; delta V T H equals the median over log current of backward V G minus forward V G.",
        egui::include_image!("../../assets/math/transfer-on-off-hysteresis.svg"),
    );
    guide_text(
        ui,
        "Hysteresis needs forward + reverse sweeps (≥12 points each).",
    );
}

fn tlm_guide(ui: &mut egui::Ui) {
    ui_kit::section_header(ui, "INPUT", None);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        guide_contract_row(ui, "Files", ".xlsx");
        guide_code(ui, "root/\n  group/\n    50/\n      device.xlsx");
        guide_contract_row(ui, "Length", "Number folder = channel length (μm).");
        guide_contract_row(ui, "Data", "List(*) sheet: vg · abs_id · abs_is");
        guide_contract_row(
            ui,
            "Bias",
            "Setup(*) sheet: V<sub>D</sub>; else Fallback V<sub>D</sub>.",
        );
    });

    ui.add_space(14.0);
    ui_kit::section_header(ui, "FIT MATHEMATICS", None);
    guide_term(ui, "Current at selected gate bias");
    guide_math(
        ui,
        "j d minimizes absolute measured V G minus selected V G; channel current is the minimum of absolute I D and absolute I S at j d; I L is the maximum channel current across devices at length L; R total equals absolute V D divided by I L.",
        egui::include_image!("../../assets/math/tlm-current.svg"),
    );
    guide_term(ui, "Ordinary least squares");
    guide_math(
        ui,
        "R total of L equals m L plus b equals m L plus two R c; R c per contact equals b over two; m and b are the displayed ordinary least-squares estimators; R squared equals one minus residual sum of squares over total sum of squares.",
        egui::include_image!("../../assets/math/tlm-regression.svg"),
    );
    guide_text(
        ui,
        "Need ≥2 lengths (≥3 for R<sup>2</sup>). Primary: highest-current device per L; median: diagnostic. m is slope (Ω/μm), not sheet resistance.",
    );
}

fn model_fit_guide(ui: &mut egui::Ui, model: usize) {
    ui_kit::section_header(ui, "INPUT", None);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        guide_contract_row(ui, "Files", ".csv · .tsv · .txt · .xlsx · .xls");
        guide_contract_row(
            ui,
            "Transfer",
            "V<sub>G</sub> · I<sub>D</sub>. Set its V<sub>DS</sub> in Parameters.",
        );
        guide_contract_row(
            ui,
            "Output",
            "V<sub>G</sub> · V<sub>D</sub> · I<sub>D</sub>.",
        );
        if model == 1 {
            guide_contract_row(
                ui,
                "DIBL",
                "Second transfer with V<sub>G</sub> · V<sub>D</sub> · I<sub>D</sub>.",
            );
        }
        guide_contract_row(ui, "C-V", "Bias · capacitance. Updates C<sub>ox</sub>.");
    });
    guide_text(
        ui,
        if model == 1 {
            "Output, DIBL, and C-V files are optional."
        } else {
            "Output and C-V files are optional."
        },
    );

    ui.add_space(14.0);
    ui_kit::section_header(ui, "MODEL EQUATIONS", None);
    if model == 1 {
        level62_equations(ui);
    } else {
        aostft_equations(ui);
    }
}

fn aostft_equations(ui: &mut egui::Ui) {
    guide_term(ui, "UMEM H-function extraction");
    guide_math(
        ui,
        "H of V G equals the integral from the first gate voltage to V G of absolute I D d u, divided by absolute I D of V G, and equals a V G plus b; V T equals minus b over a; gamma H equals one over a minus two; K H is the median of absolute I D divided by V G minus V T to the one plus gamma H power, where current is at least one percent of peak and V G is above V T.",
        egui::include_image!("../../assets/math/aostft-h-function.svg"),
    );
    guide_text(ui, "The transfer sweep must include the off region.");
    guide_term(ui, "Above-/subthreshold transfer crossover");
    guide_math(
        ui,
        "x equals V G S minus V T; u delta of x equals x plus square root of x squared plus delta squared, over two; I A H equals K H times u delta to the one plus gamma H; D V equals the square root of max of p S over natural log ten squared minus delta squared and zero; Q equals two over S; I B equals I A at D V and V D times ten to the x minus D V over S; w equals Q times x minus D V; I D is the tanh-weighted sum of I A and I B plus I off.",
        egui::include_image!("../../assets/math/aostft-crossover.svg"),
    );
    guide_text(
        ui,
        "Value is matched. Transfer-only slope is exact when the radicand ≥ 0; with output data it is approximate.",
    );
    guide_term(ui, "Output-attached card mapping");
    guide_math(
        ui,
        "Gamma c equals gamma H minus one; K c equals K H times transfer drain bias divided by alpha sat times one plus lambda times transfer drain bias; G zero equals K c over transfer drain bias; K P equals G zero times fit length over fit width; instance conductance gain equals W over L times K P.",
        egui::include_image!("../../assets/math/aostft-output-mapping.svg"),
    );
    guide_text(
        ui,
        "Use the measured transfer V<sub>DS</sub>; this mapping assumes saturation.",
    );
    guide_term(ui, "Finite-VDS channel current");
    guide_math(
        ui,
        "Channel conductance equals G zero times u delta of x to the one plus gamma c, divided by one plus R S times that numerator; drain saturation voltage equals alpha sat times u delta; effective drain voltage equals V D divided by one plus absolute V D over drain saturation voltage to the m power, all to the one over m power; I A equals channel conductance times effective drain voltage times one plus lambda V D.",
        egui::include_image!("../../assets/math/aostft-finite-drain.svg"),
    );
    guide_text(
        ui,
        "R<sub>S</sub> is fixed at zero. Exported current scales with W/L.",
    );
    guide_term(ui, "Analog terminal charge");
    guide_math(
        ui,
        "a equals u zero point two of x; b equals u zero point zero five of a minus V D; Q g equals two thirds C ox W L times a squared plus a b plus b squared, divided by a plus b plus ten to minus nine volts; drain fraction equals b squared divided by a squared plus b squared plus ten to minus eighteen volts squared; Q d equals minus drain fraction Q g and Q s equals minus one minus drain fraction times Q g.",
        egui::include_image!("../../assets/math/aostft-charge.svg"),
    );
    guide_text(
        ui,
        "C<sub>ox</sub> = 0 disables terminal charge; otherwise export contributes dQ/dt.",
    );
}

fn level62_equations(ui: &mut egui::Ui) {
    guide_term(ui, "Electrostatics + mobility");
    guide_math(
        ui,
        "Level 62-derived equations: thermal voltage equals k T over q; V s t h equals ETA times thermal voltage; C ox equals EPSI epsilon zero over TOX; V T X and mu one use delta T; V T effective applies the displayed A T and B T drain-bias shift; V G T equals V G S minus V T effective; the displayed clamp defines V G T E; inverse mu F E T is the sum of inverse MU zero and the field-dependent inverse mobility; alpha sat and V D sat follow the displayed equations.",
        egui::include_image!("../../assets/math/level62-electrostatics.svg"),
    );
    guide_text(
        ui,
        "V<sub>th</sub> is thermal voltage; VTO is zero-bias threshold. Here L<sub>eff</sub> = L. Fit uses TNOM; export uses simulator temperature.",
    );
    guide_term(ui, "Channel branches + stabilized crossover");
    guide_math(
        ui,
        "Level 62-derived equations: I a is the displayed piecewise linear-region or saturation current times one plus lambda V D S; I sub equals MUS C ox W over L times V s t h squared, times exp V G T over V s t h, times one minus exp minus V D S over V s t h; I min and I max select the branches; I channel equals I min divided by one plus square root of I min over I max squared; I D S equals I channel plus leakage, times one plus kink.",
        egui::include_image!("../../assets/math/level62-current.svg"),
    );
    guide_text(
        ui,
        "The export solves internal V<sub>GS</sub> and V<sub>DS</sub> after R<sub>S</sub>/R<sub>D</sub> drops.",
    );
    guide_term(ui, "Leakage + impact-ionization kink");
    guide_math(
        ui,
        "Level 62-derived equations: leakage equals I zero zero W times exp minus E B over thermal voltage, times one minus exp minus V D S over thermal voltage; V D s k is the displayed softened drain voltage minus thermal voltage; kink is the displayed V KINK, L KINK, M K impact-ionization expression; internal V G S equals external V G S minus I D R S; internal V D S equals external V D S minus I D times R S plus R D.",
        egui::include_image!("../../assets/math/level62-leakage-kink.svg"),
    );
    guide_text(
        ui,
        "I<sub>kink</sub> = 0 until V<sub>DS</sub> − V<sub>Dsk</sub> exceeds its numerical guard.",
    );
    guide_term(ui, "Analog terminal charge");
    guide_math(
        ui,
        "Level 62-derived charge equations: x q equals V G S minus V T O; the displayed DELTA clamp defines V G T E q; u delta of z equals z plus square root of z squared plus delta squared, over two; a equals V G T E q and b equals u zero point zero five volts of a minus V D; Q g and the drain fraction use the displayed Meyer equations with ten to minus nine volt and ten to minus eighteen volt squared guards; Q d and Q s partition minus Q g.",
        egui::include_image!("../../assets/math/level62-charge.svg"),
    );
    guide_text(
        ui,
        "Export contributes dQ/dt using C<sub>ox</sub> = EPSI·ε<sub>0</sub>/TOX. DIBL, mobility, and kink affect current only.",
    );
}

fn guide_term(ui: &mut egui::Ui, text: &str) {
    let color = if ui.is_enabled() {
        crate::theme::tokens().ink
    } else {
        crate::theme::tokens().ink_soft
    };
    let mut job = crate::richtext::layout_sub_sup(
        text,
        egui::FontId::new(13.5, ui_kit::bold_family(ui)),
        color,
    );
    job.wrap.max_width = ui.available_width();
    ui.label(job);
}

fn guide_text(ui: &mut egui::Ui, markup: &str) {
    let color = if ui.is_enabled() {
        crate::theme::tokens().ink
    } else {
        crate::theme::tokens().ink_soft
    };
    let mut job = crate::richtext::layout_sub_sup(markup, egui::FontId::proportional(13.0), color);
    job.wrap.max_width = ui.available_width().min(GUIDE_TEXT_WIDTH);
    ui.label(job);
}

fn guide_contract_text(ui: &mut egui::Ui, markup: &str) {
    let width = ui.available_width().min(GUIDE_CONTRACT_WIDTH);
    ui.allocate_ui_with_layout(
        egui::vec2(width, 0.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(width);
            guide_text(ui, markup);
        },
    );
}

fn guide_contract_row(ui: &mut egui::Ui, term: &str, markup: &str) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.allocate_ui_with_layout(
            egui::vec2(GUIDE_CONTRACT_KEY_WIDTH, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| guide_term(ui, term),
        );
        guide_contract_text(ui, markup);
    });
}

fn guide_code(ui: &mut egui::Ui, markup: &str) {
    let outer_width = ui.available_width();
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(9, 6))
        .show(ui, |ui| {
            ui.set_min_width((outer_width - 18.0).max(0.0));
            let mut job = crate::richtext::layout_sub_sup(
                markup,
                egui::FontId::monospace(12.0),
                crate::theme::tokens().ink,
            );
            job.wrap.max_width = ui.available_width();
            ui.label(job);
        });
}

fn guide_math(ui: &mut egui::Ui, alt: &str, source: egui::ImageSource<'static>) {
    let outer_width = ui.available_width();
    let frame = egui::Frame::new()
        .fill(crate::theme::token_alpha(
            crate::theme::tokens().primary,
            14,
        ))
        .stroke(egui::Stroke::new(
            1.0_f32,
            crate::theme::token_alpha(crate::theme::tokens().primary, 28),
        ))
        .corner_radius(egui::CornerRadius::same(5))
        .inner_margin(egui::Margin::symmetric(12, 9));
    let width = (outer_width - frame.total_margin().sum().x).max(0.0);
    frame.show(ui, |ui| {
        ui.set_min_width(width);
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add(
                egui::Image::new(source)
                    .alt_text(alt)
                    .fit_to_original_size(1.0)
                    .max_width(width),
            );
        });
    });
}
