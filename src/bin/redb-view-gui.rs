//! Desktop window for browsing a local `.redb` file.
//!
//! Uses [`redb_view::DatabaseView`] only — never imports `redb` directly.

use std::fs;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText, ScrollArea, Visuals};
use eframe::{NativeOptions, Theme};
use redb_view::{DatabaseView, DisplayValue, KvRow, TableInfo, TableKind, ViewError};

fn main() -> eframe::Result<()> {
    let mut options = NativeOptions::default();
    // Needed on Linux so OS theme changes reach `frame.info().system_theme`.
    options.follow_system_theme = true;
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
}

impl App {
    fn new() -> Self {
        let theme = load_theme_choice();
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
                                    .selectable_label(
                                        self.theme == choice,
                                        choice.as_label(),
                                    )
                                    .clicked()
                                {
                                    self.set_theme(choice);
                                }
                            }
                        });
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
            ui.label(RichText::new(&self.status).color(Color32::from_rgb(40, 40, 40)));
            ui.add_space(6.0);
        });

        egui::SidePanel::left("tables")
            .resizable(true)
            .default_width(220.0)
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

            ui.separator();

            ScrollArea::vertical()
                .id_source("row_list")
                .max_height(280.0)
                .show(ui, |ui| {
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

            ui.separator();
            ui.heading("Details");
            ScrollArea::vertical().id_source("detail").show(ui, |ui| {
                match self.selected_row.and_then(|i| self.rows.get(i)) {
                    Some(row) => {
                        ui.label(RichText::new(format!("Row #{}", row.index)).strong());
                        ui.add_space(6.0);
                        ui.label(RichText::new("Key").strong());
                        show_display_value(ui, &row.key);
                        ui.add_space(8.0);
                        ui.label(RichText::new("Value").strong());
                        show_display_value(ui, &row.value);
                    }
                    None => {
                        ui.label("Click a row to see the full key and value.");
                    }
                }
            });
        });
    }
}

fn preview_value(value: &DisplayValue) -> String {
    const MAX: usize = 48;
    let source = match value.text.as_deref() {
        Some(text) if !text.is_empty() => text,
        _ => value.hex.as_str(),
    };
    if source.chars().count() <= MAX {
        return source.to_owned();
    }
    let trimmed: String = source.chars().take(MAX).collect();
    format!("{trimmed}…")
}

fn show_display_value(ui: &mut egui::Ui, value: &DisplayValue) {
    ui.label(format!("Size: {} bytes", value.raw_len));
    match value.text.as_deref() {
        Some(text) if !text.is_empty() => {
            ui.label("Text:");
            ui.label(RichText::new(text).monospace());
        }
        _ => {
            ui.label("Text: (not readable as text)");
        }
    }
    ui.label("Hex:");
    ui.label(RichText::new(&value.hex).monospace());
}

fn plain_error(lead: &str, err: &ViewError) -> String {
    format!("{lead} ({err})")
}

fn theme_config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join(".config").join("redb-view").join("theme"))
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
