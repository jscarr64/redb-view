//! Desktop window for browsing a local `.redb` file.
//!
//! Uses [`redb_view::DatabaseView`] only — never imports `redb` directly.

use std::fs;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, CursorIcon, RichText, ScrollArea, Sense, TextWrapMode, Vec2, Visuals};
use eframe::{NativeOptions, Theme};
use redb_view::{DatabaseView, DisplayValue, KvRow, TableInfo, TableKind, ViewError};

const SPLITTER_THICKNESS: f32 = 6.0;
const MIN_ROW_PANEL_HEIGHT: f32 = 100.0;
const MIN_DETAIL_PANEL_HEIGHT: f32 = 100.0;
const DEFAULT_ZOOM: f32 = 1.35;
const MIN_ZOOM: f32 = 0.85;
const MAX_ZOOM: f32 = 2.5;
const ZOOM_STEP: f32 = 0.1;
const C0_DIVIDER: &str = " | ";

fn main() -> eframe::Result<()> {
    // follow_system_theme: needed on Linux so OS theme reaches frame.info().system_theme.
    let options = NativeOptions {
        follow_system_theme: true,
        ..NativeOptions::default()
    };
    eframe::run_native(
        "redb View",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeChoice {
    Light,
    Dark,
    System,
}

impl ThemeChoice {
    fn as_label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::System => "System",
        }
    }

    fn from_stored(raw: &str) -> Self {
        match raw.trim() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    fn to_stored(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }
}

struct App {
    path: String,
    db: Option<DatabaseView>,
    tables: Vec<TableInfo>,
    selected_table: Option<String>,
    offset: u64,
    page_size: usize,
    rows: Vec<KvRow>,
    selected_row: Option<usize>,
    status: String,
    theme: ThemeChoice,
    /// Height of the row-list pane above the horizontal splitter.
    row_panel_height: f32,
    /// Global UI zoom (`pixels_per_point` multiplier around egui’s baseline).
    zoom: f32,
}

impl App {
    fn new() -> Self {
        let theme = load_theme_choice();
        let zoom = load_zoom();
        Self {
            path: String::new(),
            db: None,
            tables: Vec::new(),
            selected_table: None,
            offset: 0,
            page_size: 25,
            rows: Vec::new(),
            selected_row: None,
            status: "Press Open database and choose a .redb or .db file.".to_owned(),
            theme,
            row_panel_height: 280.0,
            zoom,
        }
    }

    fn pick_and_open(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Database", &["redb", "db"])
            .add_filter("All files", &["*"])
            .set_title("Open database")
            .pick_file();

        let Some(path) = picked else {
            self.status = "Open cancelled. No file selected.".to_owned();
            return;
        };

        self.path = path.display().to_string();
        self.open_database();
    }

    fn open_database(&mut self) {
        self.db = None;
        self.tables.clear();
        self.selected_table = None;
        self.rows.clear();
        self.selected_row = None;
        self.offset = 0;

        let path = self.path.trim();
        if path.is_empty() {
            self.status = "Press Open database and choose a file.".to_owned();
            return;
        }

        match DatabaseView::open(path) {
            Ok(db) => match db.tables() {
                Ok(tables) => {
                    let count = tables.len();
                    self.db = Some(db);
                    self.tables = tables;
                    self.status = format!("Opened. {count} table(s) found.");
                }
                Err(err) => {
                    self.status = plain_error("Could not read the table list.", &err);
                }
            },
            Err(err) => {
                self.status = plain_error("Could not open that file.", &err);
            }
        }
    }

    fn select_table(&mut self, name: String) {
        self.selected_table = Some(name);
        self.offset = 0;
        self.selected_row = None;
        self.load_page();
    }

    fn load_page(&mut self) {
        self.rows.clear();
        self.selected_row = None;

        let Some(name) = self.selected_table.clone() else {
            self.status = "Pick a table first.".to_owned();
            return;
        };

        if let Some(info) = self.tables.iter().find(|t| t.name == name) {
            if info.kind == TableKind::Multimap {
                self.status =
                    "This table type (multimap) cannot be paged yet. Pick another table.".to_owned();
                return;
            }
        }

        let Some(db) = self.db.as_ref() else {
            self.status = "Open a database first.".to_owned();
            return;
        };

        match db.page(&name, self.offset, self.page_size) {
            Ok(rows) => {
                let shown = rows.len();
                self.rows = rows;
                if shown == 0 {
                    self.status = format!("No rows on this page for “{name}”.");
                } else {
                    self.status = format!(
                        "Showing rows {}–{} of table “{name}”.",
                        self.offset + 1,
                        self.offset + shown as u64
                    );
                }
            }
            Err(err) => {
                self.status = plain_error("Could not load rows for that table.", &err);
            }
        }
    }

    fn go_prev_page(&mut self) {
        if self.offset == 0 {
            return;
        }
        let step = self.page_size as u64;
        self.offset = self.offset.saturating_sub(step);
        self.load_page();
    }

    fn go_next_page(&mut self) {
        if self.rows.len() < self.page_size {
            return;
        }
        self.offset = self.offset.saturating_add(self.page_size as u64);
        self.load_page();
    }

    fn set_theme(&mut self, theme: ThemeChoice) {
        self.theme = theme;
        save_theme_choice(theme);
    }

    fn zoom_in(&mut self) {
        self.zoom = ((self.zoom + ZOOM_STEP) * 20.0).round() / 20.0;
        self.zoom = self.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        save_zoom(self.zoom);
    }

    fn zoom_out(&mut self) {
        self.zoom = ((self.zoom - ZOOM_STEP) * 20.0).round() / 20.0;
        self.zoom = self.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        save_zoom(self.zoom);
    }

    fn apply_theme(&self, ctx: &egui::Context, frame: &eframe::Frame) {
        let visuals = match self.theme {
            ThemeChoice::Light => Visuals::light(),
            ThemeChoice::Dark => Visuals::dark(),
            ThemeChoice::System => match frame.info().system_theme {
                Some(Theme::Dark) => Visuals::dark(),
                Some(Theme::Light) | None => Visuals::light(),
            },
        };
        ctx.set_visuals(visuals);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.apply_theme(ctx, frame);
        ctx.set_pixels_per_point(self.zoom);

        egui::TopBottomPanel::top("open_bar").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading("redb View");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_source("theme_choice")
                        .selected_text(format!("Theme: {}", self.theme.as_label()))
                        .show_ui(ui, |ui| {
                            for choice in [
                                ThemeChoice::System,
                                ThemeChoice::Light,
                                ThemeChoice::Dark,
                            ] {
                                if ui
                                    .selectable_label(self.theme == choice, choice.as_label())
                                    .clicked()
                                {
                                    self.set_theme(choice);
                                }
                            }
                        });
                    ui.add_space(8.0);
                    ui.label(format!("{}%", (self.zoom * 100.0).round() as i32));
                    if ui.button("Zoom out").clicked() {
                        self.zoom_out();
                    }
                    if ui.button("Zoom in").clicked() {
                        self.zoom_in();
                    }
                });
            });
            ui.label("Open a local database file, pick a table, then browse its rows.");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let open = ui.add_sized([160.0, 32.0], egui::Button::new("Open database"));
                if open.clicked() {
                    self.pick_and_open();
                }
                if self.path.is_empty() {
                    ui.label("No file selected yet.");
                } else {
                    ui.label(format!("File: {}", self.path));
                }
            });
            ui.add_space(4.0);
            ui.label(RichText::new(&self.status).weak());
            ui.add_space(6.0);
        });

        // Left/right: tables vs main view — native egui resizable side panel.
        egui::SidePanel::left("tables")
            .resizable(true)
            .default_width(220.0)
            .width_range(140.0..=520.0)
            .show(ctx, |ui| {
                ui.heading("Tables");
                if self.tables.is_empty() {
                    ui.label("No tables yet.");
                    return;
                }
                let mut clicked: Option<String> = None;
                ScrollArea::vertical().show(ui, |ui| {
                    for info in &self.tables {
                        let kind = match info.kind {
                            TableKind::Normal => "table",
                            TableKind::Multimap => "multimap",
                        };
                        let label = format!("{}  ({kind}, {} rows)", info.name, info.len);
                        let selected = self.selected_table.as_deref() == Some(info.name.as_str());
                        if ui.selectable_label(selected, label).clicked() {
                            clicked = Some(info.name.clone());
                        }
                    }
                });
                if let Some(name) = clicked {
                    self.select_table(name);
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Rows");
                ui.add_space(12.0);
                let prev = ui.add_enabled(self.offset > 0, egui::Button::new("Previous page"));
                if prev.clicked() {
                    self.go_prev_page();
                }
                let next = ui.add_enabled(
                    self.rows.len() >= self.page_size,
                    egui::Button::new("Next page"),
                );
                if next.clicked() {
                    self.go_next_page();
                }
            });
            ui.add_space(4.0);

            let available = ui.available_size();
            let max_row_height =
                (available.y - SPLITTER_THICKNESS - MIN_DETAIL_PANEL_HEIGHT).max(MIN_ROW_PANEL_HEIGHT);
            self.row_panel_height = self
                .row_panel_height
                .clamp(MIN_ROW_PANEL_HEIGHT, max_row_height);

            // Top: row list — full width, horizontal + vertical scroll, no early "…"
            ui.allocate_ui_with_layout(
                Vec2::new(available.x, self.row_panel_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ScrollArea::both()
                        .id_source("row_list")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
                            ui.set_min_width(ui.available_width());
                            if self.rows.is_empty() {
                                ui.label("No rows to show.");
                                return;
                            }
                            for (i, row) in self.rows.iter().enumerate() {
                                let preview = format!(
                                    "#{}  key: {}  |  value: {}",
                                    row.index,
                                    preview_value(&row.key),
                                    preview_value(&row.value)
                                );
                                let selected = self.selected_row == Some(i);
                                if ui.selectable_label(selected, preview).clicked() {
                                    self.selected_row = Some(i);
                                }
                            }
                        });
                },
            );

            // Horizontal splitter (row list ↔ detail)
            let (split_rect, split_response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), SPLITTER_THICKNESS),
                Sense::click_and_drag(),
            );
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            ui.painter()
                .hline(split_rect.x_range(), split_rect.center().y, stroke);
            if split_response.hovered() || split_response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
            }
            if split_response.dragged() {
                self.row_panel_height = (self.row_panel_height + split_response.drag_delta().y)
                    .clamp(MIN_ROW_PANEL_HEIGHT, max_row_height);
            }

            // Bottom: detail (UTF-8 text only — never hex; C0 sanitized for display)
            ui.heading("Details");
            ScrollArea::both()
                .id_source("detail")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
                    match self.selected_row.and_then(|i| self.rows.get(i)) {
                        Some(row) => {
                            ui.label(RichText::new(format!("Row #{}", row.index)).strong());
                            ui.add_space(6.0);
                            ui.label(RichText::new("Key").strong());
                            show_text_value(ui, &row.key);
                            ui.add_space(8.0);
                            ui.label(RichText::new("Value").strong());
                            show_text_value(ui, &row.value);
                        }
                        None => {
                            ui.label("Click a row to see the full key and value.");
                        }
                    }
                });
        });
    }
}

/// Display-only: keep tab/LF/CR; replace other C0 controls (e.g. U+001F) with a divider.
fn sanitize_for_display(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_control() && ch != '\t' && ch != '\n' && ch != '\r' {
            out.push_str(C0_DIVIDER);
        } else {
            out.push(ch);
        }
    }
    out
}

fn preview_value(value: &DisplayValue) -> String {
    match value.text.as_deref() {
        Some(text) if !text.is_empty() => sanitize_for_display(text),
        _ => "not text".to_owned(),
    }
}

fn show_text_value(ui: &mut egui::Ui, value: &DisplayValue) {
    ui.label(format!("Size: {} bytes", value.raw_len));
    match value.text.as_deref() {
        Some(text) if !text.is_empty() => {
            let shown = sanitize_for_display(text);
            ui.label(RichText::new(shown).monospace());
        }
        Some(_) => {
            ui.label(
                RichText::new("Empty text.")
                    .italics()
                    .color(Color32::GRAY),
            );
        }
        None => {
            ui.label(
                RichText::new("Not text — these bytes are not valid UTF-8.")
                    .italics()
                    .color(Color32::GRAY),
            );
        }
    }
}

fn plain_error(lead: &str, err: &ViewError) -> String {
    format!("{lead} ({err})")
}

fn config_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join(".config").join("redb-view"))
}

fn theme_config_path() -> Option<PathBuf> {
    Some(config_dir()?.join("theme"))
}

fn zoom_config_path() -> Option<PathBuf> {
    Some(config_dir()?.join("zoom"))
}

fn load_theme_choice() -> ThemeChoice {
    let Some(path) = theme_config_path() else {
        return ThemeChoice::System;
    };
    match fs::read_to_string(path) {
        Ok(raw) => ThemeChoice::from_stored(&raw),
        Err(_) => ThemeChoice::System,
    }
}

fn save_theme_choice(theme: ThemeChoice) {
    let Some(path) = theme_config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, theme.to_stored());
}

fn load_zoom() -> f32 {
    let Some(path) = zoom_config_path() else {
        return DEFAULT_ZOOM;
    };
    match fs::read_to_string(path) {
        Ok(raw) => raw
            .trim()
            .parse::<f32>()
            .ok()
            .map(|z| z.clamp(MIN_ZOOM, MAX_ZOOM))
            .unwrap_or(DEFAULT_ZOOM),
        Err(_) => DEFAULT_ZOOM,
    }
}

fn save_zoom(zoom: f32) {
    let Some(path) = zoom_config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, format!("{zoom:.2}"));
}
