use std::path::PathBuf;
use anyhow::Result;
use eframe::{egui, CreationContext};
use egui::{Context, Ui, ViewportCommand};
use rfd::FileDialog;
use dirs;

use super::pdf_viewer::PdfViewer;
use super::search_panel::SearchPanel;
use super::analysis_panel::AnalysisPanel;

/// The main application state
pub struct PdfScanApp {
    // UI state
    current_tab: Tab,
    show_sidebar: bool,
    sidebar_width: f32,
    
    // PDF viewer
    pdf_viewer: PdfViewer,
    
    // Search functionality
    search_panel: SearchPanel,
    
    // Analysis functionality
    analysis_panel: AnalysisPanel,
    
    // Global state
    recent_files: Vec<PathBuf>,
    theme: Theme,
}

#[derive(PartialEq)]
enum Tab {
    Viewer,
    Search,
    Analysis,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Theme {
    Light,
    Dark,
}

impl PdfScanApp {
    pub fn new(cc: &CreationContext) -> Self {
        // Apply custom theme
        super::theme::setup_custom_theme(&cc.egui_ctx);
        
        // Load recent files if available
        let recent_files = load_recent_files().unwrap_or_default();

        // Create the app
        Self {
            current_tab: Tab::Viewer,
            show_sidebar: true,
            sidebar_width: 250.0,
            pdf_viewer: PdfViewer::new(),
            search_panel: SearchPanel::new(),
            analysis_panel: AnalysisPanel::new(),
            recent_files,
            theme: Theme::Dark,
        }
    }
    
    /// Draw the top menu bar
    fn menu_bar(&mut self, ui: &mut Ui, ctx: &Context) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open PDF...").clicked() {
                    let file_path = open_file_dialog();
                    if let Some(path) = file_path {
                        self.pdf_viewer.load_pdf(&path);
                        add_to_recent_files(&mut self.recent_files, path);
                        ui.close_menu();
                    }
                }
                
                if ui.button("Load Directory...").clicked() {
                    if let Some(dir_path) = rfd::FileDialog::new().pick_folder() {
                        self.search_panel.load_directory(&dir_path);
                        self.current_tab = Tab::Search;
                        ui.close_menu();
                    }
                }
                
                ui.menu_button("Recent Files", |ui| {
                    for path in &self.recent_files {
                        if ui.button(path.file_name().unwrap_or_default().to_string_lossy().to_string()).clicked() {
                            self.pdf_viewer.load_pdf(path);
                            ui.close_menu();
                        }
                    }
                    if self.recent_files.is_empty() {
                        ui.label("No recent files");
                    }
                });
                
                ui.separator();
                if ui.button("Exit").clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            });
            
            ui.menu_button("View", |ui| {
                if ui.checkbox(&mut self.show_sidebar, "Show Sidebar").clicked() {
                    ui.close_menu();
                }
                
                if ui.radio_value(&mut self.theme, Theme::Light, "Light Theme").clicked() {
                    apply_theme(ctx, self.theme);
                    ui.close_menu();
                }
                if ui.radio_value(&mut self.theme, Theme::Dark, "Dark Theme").clicked() {
                    apply_theme(ctx, self.theme);
                    ui.close_menu();
                }
            });
            
            ui.menu_button("Tools", |ui| {
                if ui.button("Extract Text...").clicked() {
                    if let Some(current_pdf) = self.pdf_viewer.current_pdf() {
                        self.extract_text_dialog(current_pdf.clone());
                    } else {
                        // Show a message that no PDF is open
                    }
                    ui.close_menu();
                }
                
                if ui.button("Batch Process...").clicked() {
                    // Implement batch processing dialog
                    ui.close_menu();
                }
            });
            
            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    // Show about dialog
                    ui.close_menu();
                }
            });
        });
    }
    
    
    /// Extract text dialog
    fn extract_text_dialog(&mut self, pdf_path: PathBuf) {
        // Ask user where to save the file
        if let Some(save_path) = rfd::FileDialog::new()
            .set_file_name(format!("{}_text.txt", 
                pdf_path.file_name().unwrap_or_default().to_string_lossy()))
            .add_filter("Text Files", &["txt"])
            .save_file() 
        {
            // Extract text in a background thread
            std::thread::spawn(move || {
                // Convert the PathBuf to a vector of strings as required by the extract module
                let input_path = vec![pdf_path.to_string_lossy().to_string()];
                let output_file = save_path.to_string_lossy().to_string();
                
                // Use the extract module to save the text
                match crate::extract::run(&output_file, &input_path) {
                    Ok(_) => {
                        println!("Successfully extracted text to {}", output_file);
                    },
                    Err(e) => {
                        eprintln!("Error extracting text: {}", e);
                    }
                }
            });
        }
    }
}

impl eframe::App for PdfScanApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Create the top-level layout
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // Draw the menu bar
            self.menu_bar(ui, ctx);
        });

        // Draw the sidebar if enabled
        if self.show_sidebar {
            egui::SidePanel::left("sidebar")
                .resizable(true)
                .default_width(self.sidebar_width)
                .width_range(150.0..=350.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("PDFScan");
                    });
                    
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.current_tab == Tab::Viewer, "📄 Viewer").clicked() {
                            self.current_tab = Tab::Viewer;
                        }
                        if ui.selectable_label(self.current_tab == Tab::Search, "🔍 Search").clicked() {
                            self.current_tab = Tab::Search;
                        }
                        if ui.selectable_label(self.current_tab == Tab::Analysis, "📊 Analysis").clicked() {
                            self.current_tab = Tab::Analysis;
                        }
                    });
                    
                    ui.separator();
                    
                    // Display different sidebar content based on the selected tab
                    match self.current_tab {
                        Tab::Viewer => {
                            // Show document outline if available
                            self.pdf_viewer.show_outline(ui);
                        },
                        Tab::Search => {
                            // Show search options
                            self.search_panel.show_options(ui, &self.pdf_viewer);
                        },
                        Tab::Analysis => {
                            // Show analysis options
                            self.analysis_panel.show_options(ui, &self.pdf_viewer);
                        },
                    }
                    
                    // Store the current sidebar width
                    self.sidebar_width = ui.available_width();
                });
        }
        
        // Draw the main content
        egui::CentralPanel::default().show(ctx, |ui| {
            // Draw the main content based on the selected tab
            match self.current_tab {
                Tab::Viewer => {
                    self.pdf_viewer.show(ui, ctx);
                },
                Tab::Search => {
                    self.search_panel.show(ui, ctx, &mut self.pdf_viewer);
                },
                Tab::Analysis => {
                    self.analysis_panel.show(ui, ctx, &mut self.pdf_viewer);
                },
            }
        });
    }
}

/// Apply the selected theme
fn apply_theme(ctx: &Context, theme: Theme) {
    match theme {
        Theme::Light => {
            ctx.set_visuals(egui::Visuals::light());
        },
        Theme::Dark => {
            // Use our custom theme
            super::theme::setup_custom_theme(ctx);
        },
    }
}

/// Open a file dialog and return the selected file path
pub fn open_file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .add_filter("PDF Files", &["pdf"])
        .pick_file()
}

/// Load recent files from storage
fn load_recent_files() -> Result<Vec<PathBuf>> {
    // Try to load from config directory
    let config_dir = match dirs::config_dir() {
        Some(dir) => dir.join("pdfscan"),
        None => return Ok(Vec::new()),
    };
    
    // Create directory if it doesn't exist
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
        return Ok(Vec::new());
    }
    
    let recent_files_path = config_dir.join("recent_files.txt");
    
    // If file doesn't exist, return empty vector
    if !recent_files_path.exists() {
        return Ok(Vec::new());
    }
    
    // Read file contents
    let content = std::fs::read_to_string(recent_files_path)?;
    
    // Parse file paths
    let recent_files: Vec<PathBuf> = content
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| PathBuf::from(line))
        .collect();
    
    Ok(recent_files)
}

/// Save recent files to storage
fn save_recent_files(recent_files: &[PathBuf]) -> Result<()> {
    // Get config directory
    let config_dir = match dirs::config_dir() {
        Some(dir) => dir.join("pdfscan"),
        None => return Ok(()),
    };
    
    // Create directory if it doesn't exist
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
    }
    
    let recent_files_path = config_dir.join("recent_files.txt");
    
    // Convert paths to strings and join with newlines
    let content: String = recent_files
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<String>>()
        .join("\n");
    
    // Write to file
    std::fs::write(recent_files_path, content)?;
    
    Ok(())
}

/// Add a file to the recent files list
fn add_to_recent_files(recent_files: &mut Vec<PathBuf>, path: PathBuf) {
    // Remove the file if it already exists
    recent_files.retain(|p| p != &path);
    
    // Add the file to the front of the list
    recent_files.insert(0, path);
    
    // Limit the number of recent files
    if recent_files.len() > 10 {
        recent_files.truncate(10);
    }
    
    // Save the recent files list
    save_recent_files(recent_files).ok();
} 