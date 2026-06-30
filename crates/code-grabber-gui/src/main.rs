use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use code_grabber_core::{
    BundleMode, GenerationResult, ScanConfig, ScanConfigBuilder, generate_bundle,
    init_project_profile, load_config, write_bundle_file,
};
use eframe::egui::{
    self, Align, Button, CentralPanel, Color32, ComboBox, Context, CornerRadius, FontId, Frame,
    Grid, Layout, Margin, Panel, RichText, ScrollArea, Sense, Spinner, Stroke, TextEdit, Theme, Ui,
    Vec2, Visuals,
};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Code Grabber")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Code Grabber",
        options,
        Box::new(|cc| Ok(Box::new(CodeGrabberApp::new(cc)))),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewTab {
    Included,
    Excluded,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobKind {
    Inspect,
    Bundle,
    InitProfile,
}

#[derive(Debug)]
struct JobMessage {
    kind: JobKind,
    result: Result<JobPayload, String>,
}

#[derive(Debug)]
enum JobPayload {
    Generated {
        result: Box<GenerationResult>,
        run: RunContext,
        output_path: Option<PathBuf>,
        copied: bool,
        wrote_file: bool,
    },
    ProfileInitialized(PathBuf),
}

#[derive(Debug, Clone)]
struct RunContext {
    root: PathBuf,
    mode: BundleMode,
    budget: Option<usize>,
}

struct CodeGrabberApp {
    root: String,
    output_path: String,
    mode: BundleMode,
    token_budget: String,
    include_tests: bool,
    include_docs: bool,
    include_lockfiles: bool,
    include_hidden: bool,
    strip_comments: bool,
    copy_to_clipboard: bool,
    write_to_file: bool,
    active_tab: ViewTab,
    file_filter: String,
    last_result: Option<GenerationResult>,
    last_run: Option<RunContext>,
    output_preview: String,
    status: String,
    error: Option<String>,
    last_profile_path: Option<PathBuf>,
    receiver: Option<Receiver<JobMessage>>,
    running_job: Option<JobKind>,
    job_started_at: Option<Instant>,
}

impl CodeGrabberApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_style(&cc.egui_ctx);

        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let output_path = dirs::download_dir()
            .unwrap_or_else(|| root.clone())
            .join("codebase_bundle.txt");

        Self {
            root: root.display().to_string(),
            output_path: output_path.display().to_string(),
            mode: BundleMode::Core,
            token_budget: "250000".to_string(),
            include_tests: false,
            include_docs: false,
            include_lockfiles: false,
            include_hidden: false,
            strip_comments: false,
            copy_to_clipboard: true,
            write_to_file: true,
            active_tab: ViewTab::Included,
            file_filter: String::new(),
            last_result: None,
            last_run: None,
            output_preview: String::new(),
            status: "Ready to scan a repository.".to_string(),
            error: None,
            last_profile_path: None,
            receiver: None,
            running_job: None,
            job_started_at: None,
        }
    }

    fn poll_jobs(&mut self, ctx: &Context) {
        let Some(receiver) = &self.receiver else {
            return;
        };

        match receiver.try_recv() {
            Ok(message) => {
                self.receiver = None;
                self.running_job = None;
                self.job_started_at = None;
                self.error = None;

                match message.result {
                    Ok(JobPayload::Generated {
                        result,
                        run,
                        output_path,
                        copied,
                        wrote_file,
                    }) => {
                        let result = *result;
                        let included = result.report.included_files.len();
                        let tokens = result.report.total_tokens;
                        let suffix = match message.kind {
                            JobKind::Inspect => "Scan complete",
                            JobKind::Bundle => "Bundle complete",
                            JobKind::InitProfile => "Done",
                        };
                        self.status = format!(
                            "{suffix}: {included} files, {} tokens{}{}.",
                            format_number(tokens),
                            if wrote_file { ", wrote output" } else { "" },
                            if copied { ", copied to clipboard" } else { "" }
                        );
                        self.output_preview = preview_text(&result.output.contents);
                        self.last_result = Some(result);
                        self.last_run = Some(run);
                        if let Some(path) = output_path {
                            self.output_path = path.display().to_string();
                        }
                    }
                    Ok(JobPayload::ProfileInitialized(path)) => {
                        self.status = format!("Profile initialized at {}.", path.display());
                        self.last_profile_path = Some(path);
                    }
                    Err(err) => {
                        self.error = Some(err);
                        self.status = "The last operation failed.".to_string();
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.receiver = None;
                self.running_job = None;
                self.job_started_at = None;
                self.error =
                    Some("Worker thread disconnected before returning a result.".to_string());
            }
        }
    }

    fn selected_config(&self, write_file: bool) -> Result<ScanConfig, String> {
        let root = PathBuf::from(self.root.trim());
        if self.root.trim().is_empty() {
            return Err("Choose a repository root before scanning.".to_string());
        }

        let budget = if self.token_budget.trim().is_empty() {
            None
        } else {
            Some(
                self.token_budget
                    .trim()
                    .replace(',', "")
                    .parse::<usize>()
                    .map_err(|_| "Token budget must be a whole number.".to_string())?,
            )
        };

        let output = if self.output_path.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(self.output_path.trim()))
        };

        let mut config = load_config(&root).or_else(|_| {
            ScanConfigBuilder::new(&root)
                .build()
                .map_err(|err| err.to_string())
        })?;

        config.mode = self.mode;
        config.token_budget = budget;
        config.include_rules.tests = self.include_tests;
        config.include_rules.docs = self.include_docs;
        config.include_rules.lockfiles = self.include_lockfiles;
        config.include_rules.hidden_files = self.include_hidden;
        config.strip_rules.comments = self.strip_comments;
        config.output.copy_to_clipboard = write_file && self.copy_to_clipboard;
        config.output.write_to_file = write_file && self.write_to_file;

        if let Some(path) = output {
            config.output.output_dir = path.parent().unwrap_or_else(|| ".".as_ref()).to_path_buf();
            config.output.output_filename = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "codebase_bundle.txt".to_string());
        }

        config.finalize().map_err(|err| err.to_string())
    }

    fn launch_generate(&mut self, kind: JobKind) {
        if self.running_job.is_some() {
            return;
        }

        let write_file = matches!(kind, JobKind::Bundle);
        let config = match self.selected_config(write_file) {
            Ok(config) => config,
            Err(err) => {
                self.error = Some(err);
                return;
            }
        };
        let run = RunContext {
            root: config.root.clone(),
            mode: config.mode,
            budget: config.token_budget,
        };

        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.running_job = Some(kind);
        self.job_started_at = Some(Instant::now());
        self.error = None;
        self.status = match kind {
            JobKind::Inspect => "Scanning repository...".to_string(),
            JobKind::Bundle => "Generating bundle...".to_string(),
            JobKind::InitProfile => "Initializing profile...".to_string(),
        };

        thread::spawn(move || {
            let result = generate_bundle(&config)
                .map_err(|err| err.to_string())
                .and_then(|result| {
                    let output_path = config.output_path();
                    let wrote_file = config.output.write_to_file;
                    if wrote_file {
                        write_bundle_file(&output_path, &result.output.contents)
                            .map_err(|err| err.to_string())?;
                    }

                    let copied = config.output.copy_to_clipboard;
                    if copied {
                        let mut clipboard = arboard::Clipboard::new()
                            .map_err(|err| format!("failed to open clipboard: {err}"))?;
                        clipboard
                            .set_text(result.output.contents.clone())
                            .map_err(|err| format!("failed to copy bundle: {err}"))?;
                    }

                    Ok(JobPayload::Generated {
                        result: Box::new(result),
                        run,
                        output_path: Some(output_path),
                        copied,
                        wrote_file,
                    })
                });

            let _ = sender.send(JobMessage { kind, result });
        });
    }

    fn launch_init_profile(&mut self) {
        if self.running_job.is_some() {
            return;
        }

        let root = PathBuf::from(self.root.trim());
        if self.root.trim().is_empty() {
            self.error =
                Some("Choose a repository root before initializing a profile.".to_string());
            return;
        }

        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.running_job = Some(JobKind::InitProfile);
        self.job_started_at = Some(Instant::now());
        self.error = None;
        self.status = "Initializing profile...".to_string();

        thread::spawn(move || {
            let result = init_project_profile(&root)
                .map(|_| JobPayload::ProfileInitialized(root.join(".codebundle.toml")))
                .map_err(|err| err.to_string());
            let _ = sender.send(JobMessage {
                kind: JobKind::InitProfile,
                result,
            });
        });
    }

    fn choose_root(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.root = path.display().to_string();
        }
    }

    fn choose_output(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("codebase_bundle.txt")
            .save_file()
        {
            self.output_path = path.display().to_string();
        }
    }
}

impl eframe::App for CodeGrabberApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_jobs(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        Panel::top("top_bar")
            .frame(Frame::new().fill(Color32::from_rgb(17, 24, 39)))
            .show(ui, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Code Grabber")
                                .font(FontId::proportional(24.0))
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new("Token-aware repository bundles for LLM workflows")
                                .color(Color32::from_rgb(203, 213, 225)),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let busy = self.running_job.is_some();
                        if ui
                            .add_enabled(
                                !busy,
                                Button::new("Generate Bundle").min_size(Vec2::new(140.0, 36.0)),
                            )
                            .clicked()
                        {
                            self.launch_generate(JobKind::Bundle);
                        }
                        if ui
                            .add_enabled(
                                !busy,
                                Button::new("Inspect").min_size(Vec2::new(92.0, 36.0)),
                            )
                            .clicked()
                        {
                            self.launch_generate(JobKind::Inspect);
                        }
                    });
                });
                ui.add_space(10.0);
            });

        Panel::left("settings")
            .resizable(false)
            .exact_size(330.0)
            .frame(Frame::new().fill(Color32::from_rgb(248, 250, 252)))
            .show(ui, |ui| {
                ui.add_space(16.0);
                ui.vertical_centered_justified(|ui| {
                    section_title(ui, "Repository");
                });
                ui.add_space(8.0);

                ui.label("Root");
                ui.horizontal(|ui| {
                    ui.add(TextEdit::singleline(&mut self.root).desired_width(230.0));
                    if ui.button("Browse").clicked() {
                        self.choose_root();
                    }
                });
                ui.add_space(10.0);
                ui.label("Output");
                ui.horizontal(|ui| {
                    ui.add(TextEdit::singleline(&mut self.output_path).desired_width(230.0));
                    if ui.button("Browse").clicked() {
                        self.choose_output();
                    }
                });

                ui.add_space(18.0);
                section_title(ui, "Bundle");
                ui.add_space(8.0);
                ComboBox::from_label("Mode")
                    .selected_text(mode_label(self.mode))
                    .show_ui(ui, |ui| {
                        for mode in [
                            BundleMode::Core,
                            BundleMode::Complete,
                            BundleMode::Compact,
                            BundleMode::Map,
                            BundleMode::Diff,
                            BundleMode::Pinned,
                        ] {
                            ui.selectable_value(&mut self.mode, mode, mode_label(mode));
                        }
                    });
                ui.add_space(10.0);
                ui.label("Token budget");
                ui.add(TextEdit::singleline(&mut self.token_budget).desired_width(f32::INFINITY));

                ui.add_space(18.0);
                section_title(ui, "Include");
                ui.checkbox(&mut self.include_tests, "Tests");
                ui.checkbox(&mut self.include_docs, "Documentation");
                ui.checkbox(&mut self.include_lockfiles, "Lockfiles");
                ui.checkbox(&mut self.include_hidden, "Hidden files");

                ui.add_space(18.0);
                section_title(ui, "Output");
                ui.checkbox(&mut self.strip_comments, "Strip comments where supported");
                ui.checkbox(&mut self.write_to_file, "Write bundle to file");
                ui.checkbox(&mut self.copy_to_clipboard, "Copy bundle to clipboard");

                ui.add_space(18.0);
                if ui.button("Initialize .codebundle.toml").clicked() {
                    self.launch_init_profile();
                }

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(16.0);
                    if let Some(started) = self.job_started_at {
                        let elapsed = started.elapsed().as_secs();
                        ui.horizontal(|ui| {
                            ui.add(
                                Spinner::new()
                                    .size(18.0)
                                    .color(Color32::from_rgb(37, 99, 235)),
                            );
                            ui.label(
                                RichText::new(format!("{}s elapsed", elapsed))
                                    .color(Color32::from_rgb(71, 85, 105)),
                            );
                        });
                    }
                    status_panel(ui, &self.status, self.error.is_some());
                });
            });

        CentralPanel::default()
            .frame(Frame::new().fill(Color32::from_rgb(241, 245, 249)))
            .show(ui, |ui| {
                ui.add_space(16.0);
                render_summary(
                    ui,
                    self.last_result.as_ref(),
                    self.last_run.as_ref(),
                    self.error.as_deref(),
                );
                ui.add_space(14.0);
                render_tabs(ui, &mut self.active_tab);
                ui.add_space(8.0);
                render_tab_body(
                    ui,
                    self.active_tab,
                    self.last_result.as_ref(),
                    &self.output_preview,
                    &mut self.file_filter,
                );
            });
    }
}

fn install_style(ctx: &Context) {
    ctx.set_theme(Theme::Light);
    let mut style = (*ctx.style_of(Theme::Light)).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 8.0);
    style.visuals = Visuals::light();
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(6);
    style.visuals.selection.bg_fill = Color32::from_rgb(37, 99, 235);
    ctx.set_style_of(Theme::Light, style);
}

fn section_title(ui: &mut Ui, label: &str) {
    ui.label(
        RichText::new(label)
            .strong()
            .color(Color32::from_rgb(15, 23, 42)),
    );
}

fn status_panel(ui: &mut Ui, status: &str, is_error: bool) {
    let (fill, stroke, text) = if is_error {
        (
            Color32::from_rgb(254, 242, 242),
            Color32::from_rgb(252, 165, 165),
            Color32::from_rgb(153, 27, 27),
        )
    } else {
        (
            Color32::from_rgb(239, 246, 255),
            Color32::from_rgb(191, 219, 254),
            Color32::from_rgb(30, 64, 175),
        )
    };

    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.label(RichText::new(status).color(text));
        });
}

fn render_summary(
    ui: &mut Ui,
    result: Option<&GenerationResult>,
    run: Option<&RunContext>,
    error: Option<&str>,
) {
    Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            if let Some(error) = error {
                ui.label(
                    RichText::new("Action required")
                        .font(FontId::proportional(20.0))
                        .strong()
                        .color(Color32::from_rgb(185, 28, 28)),
                );
                ui.label(error);
                return;
            }

            let Some(result) = result else {
                ui.label(
                    RichText::new("No scan yet")
                        .font(FontId::proportional(20.0))
                        .strong(),
                );
                ui.label("Ready for a repository scan.");
                return;
            };

            if let Some(run) = run {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(run.root.display().to_string())
                            .strong()
                            .color(Color32::from_rgb(15, 23, 42)),
                    );
                    ui.label(
                        RichText::new(mode_label(run.mode)).color(Color32::from_rgb(71, 85, 105)),
                    );
                });
                ui.add_space(10.0);
            }

            ui.horizontal_wrapped(|ui| {
                metric_card(
                    ui,
                    "Included",
                    &format_number(result.report.included_files.len()),
                );
                metric_card(
                    ui,
                    "Excluded",
                    &format_number(result.report.excluded_files.len()),
                );
                metric_card(ui, "Source Size", &format_bytes(result.report.total_bytes));
                metric_card(ui, "Tokens", &format_number(result.report.total_tokens));
                if let Some(budget) = run.and_then(|run| run.budget) {
                    budget_card(ui, result.report.total_tokens, budget);
                }
            });

            if !result.report.warnings.is_empty() {
                ui.add_space(12.0);
                warning_list(ui, &result.report.warnings);
            }
        });
}

fn metric_card(ui: &mut Ui, label: &str, value: &str) {
    Frame::new()
        .fill(Color32::from_rgb(248, 250, 252))
        .stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_min_width(150.0);
            ui.label(RichText::new(label).color(Color32::from_rgb(100, 116, 139)));
            ui.label(
                RichText::new(value)
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(Color32::from_rgb(15, 23, 42)),
            );
        });
}

fn budget_card(ui: &mut Ui, tokens: usize, budget: usize) {
    let (label, value, color) = if tokens > budget {
        (
            "Budget",
            format!("{} over", format_number(tokens - budget)),
            Color32::from_rgb(185, 28, 28),
        )
    } else {
        (
            "Budget",
            format!("{} left", format_number(budget - tokens)),
            Color32::from_rgb(21, 128, 61),
        )
    };

    Frame::new()
        .fill(Color32::from_rgb(248, 250, 252))
        .stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_min_width(150.0);
            ui.label(RichText::new(label).color(Color32::from_rgb(100, 116, 139)));
            ui.label(
                RichText::new(value)
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(color),
            );
        });
}

fn warning_list(ui: &mut Ui, warnings: &[String]) {
    Frame::new()
        .fill(Color32::from_rgb(255, 251, 235))
        .stroke(Stroke::new(1.0, Color32::from_rgb(253, 230, 138)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{} warning{}",
                    warnings.len(),
                    plural_suffix(warnings.len())
                ))
                .strong()
                .color(Color32::from_rgb(146, 64, 14)),
            );
            for warning in warnings.iter().take(4) {
                ui.label(RichText::new(warning).color(Color32::from_rgb(120, 53, 15)));
            }
        });
}

fn render_tabs(ui: &mut Ui, active_tab: &mut ViewTab) {
    ui.horizontal(|ui| {
        tab_button(ui, active_tab, ViewTab::Included, "Included");
        tab_button(ui, active_tab, ViewTab::Excluded, "Excluded");
        tab_button(ui, active_tab, ViewTab::Output, "Output Preview");
    });
}

fn tab_button(ui: &mut Ui, active_tab: &mut ViewTab, tab: ViewTab, label: &str) {
    let selected = *active_tab == tab;
    let response = ui.add(
        Button::new(RichText::new(label).strong())
            .selected(selected)
            .min_size(Vec2::new(130.0, 34.0)),
    );
    if response.clicked() {
        *active_tab = tab;
    }
}

fn render_tab_body(
    ui: &mut Ui,
    active_tab: ViewTab,
    result: Option<&GenerationResult>,
    output_preview: &str,
    file_filter: &mut String,
) {
    Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_height(430.0);
            match active_tab {
                ViewTab::Included => render_file_table(ui, result, true, file_filter),
                ViewTab::Excluded => render_file_table(ui, result, false, file_filter),
                ViewTab::Output => {
                    if output_preview.is_empty() {
                        ui.label("Generate or inspect a bundle to preview the output.");
                    } else {
                        ScrollArea::vertical().show(ui, |ui| {
                            let mut preview = output_preview.to_string();
                            ui.add(
                                TextEdit::multiline(&mut preview)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(24)
                                    .interactive(false),
                            );
                        });
                    }
                }
            }
        });
}

fn render_file_table(
    ui: &mut Ui,
    result: Option<&GenerationResult>,
    included: bool,
    file_filter: &mut String,
) {
    let Some(result) = result else {
        ui.label("Run an inspection to populate this table.");
        return;
    };

    let mut files = if included {
        result.report.included_files.clone()
    } else {
        result.report.excluded_files.clone()
    };

    if included {
        files.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.path.cmp(&b.path)));
    } else {
        files.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));
    }

    let total_files = files.len();
    let filter = file_filter.trim().to_lowercase();
    if !filter.is_empty() {
        files.retain(|file| {
            file.path.to_lowercase().contains(&filter)
                || file
                    .language
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&filter)
                || file
                    .reason
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&filter)
        });
    }

    let sort_label = if included {
        "Largest token counts first"
    } else {
        "Largest excluded files first"
    };
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(file_filter)
                .hint_text("Filter paths, languages, reasons")
                .desired_width(280.0),
        );
        if !file_filter.is_empty() && ui.button("Clear").clicked() {
            file_filter.clear();
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "{} file{}{}",
                format_number(files.len()),
                plural_suffix(files.len()),
                if filter.is_empty() {
                    String::new()
                } else {
                    format!(" of {}", format_number(total_files))
                }
            ))
            .strong()
            .color(Color32::from_rgb(15, 23, 42)),
        );
        ui.label(RichText::new(sort_label).color(Color32::from_rgb(100, 116, 139)));
    });
    ui.add_space(6.0);

    if files.is_empty() {
        if filter.is_empty() {
            ui.label("No files in this category.");
        } else {
            ui.label("No files match the current filter.");
        }
        return;
    }

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            Grid::new(if included {
                "included_files"
            } else {
                "excluded_files"
            })
            .striped(true)
            .min_col_width(80.0)
            .show(ui, |ui| {
                ui.label(RichText::new("Path").strong());
                ui.label(RichText::new("Language").strong());
                ui.label(RichText::new("Size").strong());
                if included {
                    ui.label(RichText::new("Tokens").strong());
                } else {
                    ui.label(RichText::new("Reason").strong());
                }
                ui.end_row();

                for file in files.iter().take(400) {
                    ui.add(egui::Label::new(truncate_middle(&file.path, 72)).sense(Sense::hover()))
                        .on_hover_text(&file.path);
                    ui.label(file.language.as_deref().unwrap_or("-"));
                    ui.label(format_bytes(file.bytes));
                    if included {
                        ui.label(format_number(file.tokens));
                    } else {
                        let reason = file.reason.as_deref().unwrap_or("excluded");
                        ui.add(egui::Label::new(truncate_end(reason, 58)).sense(Sense::hover()))
                            .on_hover_text(reason);
                    }
                    ui.end_row();
                }
            });
        });

    if files.len() > 400 {
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "Showing first 400 of {}.",
                format_number(files.len())
            ))
            .color(Color32::from_rgb(100, 116, 139)),
        );
    }
}

fn preview_text(contents: &str) -> String {
    const LIMIT: usize = 40_000;
    if contents.len() <= LIMIT {
        contents.to_string()
    } else {
        let cutoff = contents
            .char_indices()
            .map(|(index, _)| index)
            .take_while(|index| *index <= LIMIT)
            .last()
            .unwrap_or(0);
        format!(
            "{}\n\n[preview truncated at {} bytes]",
            &contents[..cutoff],
            cutoff
        )
    }
}

fn mode_label(mode: BundleMode) -> &'static str {
    match mode {
        BundleMode::Complete => "complete",
        BundleMode::Core => "core",
        BundleMode::Compact => "compact",
        BundleMode::Map => "map",
        BundleMode::Diff => "diff",
        BundleMode::Pinned => "pinned",
    }
}

fn format_number(value: usize) -> String {
    let raw = value.to_string();
    let mut out = String::with_capacity(raw.len() + raw.len() / 3);
    for (index, ch) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn format_bytes(bytes: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 10.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn truncate_middle(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }

    let keep = width - 3;
    let head = keep / 2;
    let tail = keep - head;
    let start: String = value.chars().take(head).collect();
    let end: String = value
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{start}...{end}")
}

fn truncate_end(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        value.to_string()
    } else if width <= 3 {
        ".".repeat(width)
    } else {
        format!("{}...", value.chars().take(width - 3).collect::<String>())
    }
}
