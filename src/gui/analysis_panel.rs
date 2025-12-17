use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::{Context, Ui, RichText, Color32, TextEdit, Vec2};

use super::pdf_viewer::PdfViewer;

/// Analysis panel component
pub struct AnalysisPanel {
    keywords: String,
    input_paths: Vec<PathBuf>,
    correlation_threshold: f32,
    results: Option<AnalysisResult>,
    is_analyzing: bool,
    error_message: Option<String>,
}

/// Analysis result
struct AnalysisResult {
    keywords: Vec<String>,
    correlation_matrix: Vec<Vec<f32>>,
    ranked_docs: Vec<RankedDocument>,
    total_documents: usize,
}

/// Document with rank
#[derive(Clone)]
struct RankedDocument {
    name: String,
    path: PathBuf,
    score: f32,
}

impl AnalysisPanel {
    pub fn new() -> Self {
        Self {
            keywords: String::new(),
            input_paths: Vec::new(),
            correlation_threshold: 0.1,
            results: None,
            is_analyzing: false,
            error_message: None,
        }
    }
    
    /// Show analysis options in the sidebar
    pub fn show_options(&mut self, ui: &mut Ui, pdf_viewer: &PdfViewer) {
        ui.heading("Analysis Options");
        
        // Keywords input
        ui.label("Keywords (comma separated):");
        let text_edit = TextEdit::multiline(&mut self.keywords)
            .hint_text("machine learning, neural networks, ...")
            .desired_width(ui.available_width())
            .desired_rows(3);
        
        ui.add(text_edit);
        
        ui.add_space(10.0);
        
        // Input paths
        ui.label("Input Sources:");
        
        // Add current document
        if let Some(current_pdf) = pdf_viewer.current_pdf() {
            let name = current_pdf.file_name().unwrap_or_default().to_string_lossy().to_string();
            let _path_str = current_pdf.to_string_lossy().to_string();
            
            ui.horizontal(|ui| {
                ui.label("Current:");
                ui.label(RichText::new(&name).strong());
            });
            
            if ui.button("Add Current Document").clicked() {
                if !self.input_paths.contains(current_pdf) {
                    self.input_paths.push(current_pdf.clone());
                }
            }
        }
        
        // Add directory
        if ui.button("Add Directory...").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                if !self.input_paths.contains(&path) {
                    self.input_paths.push(path);
                }
            }
        }
        
        // Show selected paths
        if !self.input_paths.is_empty() {
            ui.group(|ui| {
                ui.label(RichText::new("Selected Sources:").strong());
                
                for (i, path) in self.input_paths.clone().iter().enumerate() {
                    ui.horizontal(|ui| {
                        let name = if path.is_dir() {
                            format!("Directory: {}", path.to_string_lossy())
                        } else {
                            format!("File: {}", path.file_name().unwrap_or_default().to_string_lossy())
                        };
                        
                        ui.label(&name);
                        
                        if ui.small_button("✖").clicked() {
                            self.input_paths.remove(i);
                        }
                    });
                }
            });
        }
        
        ui.add_space(10.0);
        
        // Correlation threshold slider
        ui.label("Correlation Threshold:");
        ui.add(egui::Slider::new(&mut self.correlation_threshold, 0.0..=1.0).text("threshold"));
        
        ui.add_space(15.0);
        
        // Analyze button
        let button_text = if self.is_analyzing {
            "Analyzing..."
        } else {
            "Analyze"
        };
        
        if ui.button(button_text).clicked() && !self.is_analyzing && !self.keywords.is_empty() && !self.input_paths.is_empty() {
            self.perform_analysis();
        }
    }
    
    /// Perform analysis
    fn perform_analysis(&mut self) {
        self.is_analyzing = true;
        self.error_message = None;
        
        // Parse keywords
        let keywords: Vec<String> = self.keywords
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
            
        if keywords.is_empty() {
            self.is_analyzing = false;
            self.error_message = Some("No keywords specified".to_string());
            return;
        }
        
        // Prepare paths for analysis
        let input_paths: Vec<String> = self.input_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
            
        if input_paths.is_empty() {
            self.is_analyzing = false;
            self.error_message = Some("No input sources selected".to_string());
            return;
        }
        
        // Create clones for the thread
        let keywords_clone = keywords.clone();
        let input_paths_clone = input_paths.clone();
        let threshold = self.correlation_threshold as f64;
        
        // Create a temporary file to store analysis results
        let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
        let output_file = format!("pdf_analysis_{}.txt", timestamp);
        
        // Create shared results for thread communication
        let results = Arc::new(Mutex::new(None));
        let results_clone = results.clone();
        let error_message = Arc::new(Mutex::new(None));
        let error_message_clone = error_message.clone();
        
        // Run analysis in a background thread
        std::thread::spawn(move || {
            // Use the stats module to perform analysis
            match crate::stats::run(&input_paths_clone, &keywords_clone, &output_file, threshold) {
                Ok(_) => {
                    println!("Analysis completed and saved to {}", output_file);
                    
                    // Create analysis result data
                    let mut correlation_matrix = vec![vec![0.0; keywords_clone.len()]; keywords_clone.len()];
                    let mut ranked_docs = Vec::new();
                    
                    // Try to read the analysis file
                    match std::fs::read_to_string(&output_file) {
                        Ok(content) => {
                            // Parse correlation matrix
                            // This is a simplified parsing approach - in a real app 
                            // we would parse the file's structure more carefully
                            let mut in_matrix = false;
                            let mut in_ranked = false;
                            
                            for line in content.lines() {
                                // Skip lines with Unicode warnings
                                if line.contains("Unicode mismatch") || line.contains("unknown glyph") {
                                    continue;
                                }
                                
                                // Look for correlation matrix section
                                if line.contains("Correlation Matrix") {
                                    in_matrix = true;
                                    in_ranked = false;
                                    continue;
                                }
                                
                                // Look for ranked documents section
                                if line.contains("Ranked Documents") {
                                    in_matrix = false;
                                    in_ranked = true;
                                    continue;
                                }
                                
                                // Parse correlation matrix lines
                                if in_matrix && line.starts_with(char::is_numeric) {
                                    let parts: Vec<&str> = line.split_whitespace().collect();
                                    if parts.len() >= 3 {
                                        if let (Ok(i), Ok(j), Ok(val)) = (
                                            parts[0].parse::<usize>(),
                                            parts[1].parse::<usize>(),
                                            parts[2].parse::<f32>()
                                        ) {
                                            if i < correlation_matrix.len() && j < correlation_matrix[i].len() {
                                                correlation_matrix[i][j] = val;
                                            }
                                        }
                                    }
                                }
                                
                                // Parse ranked document lines
                                if in_ranked && line.contains("). ") {
                                    let parts: Vec<&str> = line.split("). ").collect();
                                    if parts.len() >= 2 {
                                        let name_parts: Vec<&str> = parts[1].split(" (score: ").collect();
                                        if name_parts.len() >= 2 {
                                            let name = name_parts[0].to_string();
                                            if let Ok(score) = name_parts[1].trim_end_matches(')').parse::<f32>() {
                                                // Create a path from the name (simplified approach)
                                                let path = PathBuf::from(&name);
                                                
                                                ranked_docs.push(RankedDocument {
                                                    name,
                                                    path,
                                                    score,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                            
                            // Store the results
                            let mut results = results_clone.lock().unwrap();
                            *results = Some(AnalysisResult {
                                keywords: keywords_clone,
                                correlation_matrix,
                                ranked_docs,
                                total_documents: input_paths_clone.len(),
                            });
                        },
                        Err(e) => {
                            let mut error = error_message_clone.lock().unwrap();
                            *error = Some(format!("Error reading analysis results: {}", e));
                        }
                    }
                },
                Err(e) => {
                    let mut error = error_message_clone.lock().unwrap();
                    *error = Some(format!("Error performing analysis: {}", e));
                }
            }
        });
        
        // Wait a bit for results (in a real app, we'd handle this asynchronously)
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        // Get any results so far
        let locked_results = results.lock().unwrap();
        if let Some(analysis_result) = &*locked_results {
            // Clone the result for our instance
            self.results = Some(AnalysisResult {
                keywords: analysis_result.keywords.clone(),
                correlation_matrix: analysis_result.correlation_matrix.clone(),
                ranked_docs: analysis_result.ranked_docs.clone(),
                total_documents: analysis_result.total_documents,
            });
        }
        
        // Check for errors
        let locked_error = error_message.lock().unwrap();
        if let Some(error) = &*locked_error {
            self.error_message = Some(error.clone());
        }
        
        self.is_analyzing = false;
    }
    
    /// Show the analysis panel in the main content area
    pub fn show(&mut self, ui: &mut Ui, _ctx: &Context, pdf_viewer: &mut PdfViewer) {
        ui.vertical(|ui| {
            ui.heading("Keyword Analysis");
            
            // Show error message if any
            let mut clear_error = false;
            {
                if let Some(error) = &self.error_message {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("⚠ Error:").color(Color32::RED).strong());
                        ui.label(error);
                        clear_error = ui.button("×").clicked();
                    });
                    ui.separator();
                }
            }
            
            if clear_error {
                self.error_message = None;
            }
            
            // Analysis configuration
            ui.collapsing("Analysis Configuration", |ui| {
                // Keywords
                ui.label("Keywords (comma separated):");
                ui.add(TextEdit::multiline(&mut self.keywords)
                    .hint_text("machine learning, neural networks, ...")
                    .desired_width(ui.available_width())
                    .desired_rows(2));
                
                ui.horizontal(|ui| {
                    // Input paths
                    if ui.button("Add Directory...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            if !self.input_paths.contains(&path) {
                                self.input_paths.push(path);
                            }
                        }
                    }
                    
                    if let Some(current_pdf) = pdf_viewer.current_pdf() {
                        if ui.button("Add Current Document").clicked() {
                            if !self.input_paths.contains(current_pdf) {
                                self.input_paths.push(current_pdf.clone());
                            }
                        }
                    }
                    
                    // Correlation threshold
                    ui.label("Threshold:");
                    ui.add(egui::Slider::new(&mut self.correlation_threshold, 0.0..=1.0)
                        .text("threshold")
                        .fixed_decimals(2));
                });
                
                // Selected paths
                if !self.input_paths.is_empty() {
                    ui.label(RichText::new("Selected Sources:").strong());
                    
                    egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                        for (i, path) in self.input_paths.clone().iter().enumerate() {
                            ui.horizontal(|ui| {
                                let name = if path.is_dir() {
                                    format!("Directory: {}", path.to_string_lossy())
                                } else {
                                    format!("File: {}", path.file_name().unwrap_or_default().to_string_lossy())
                                };
                                
                                ui.label(&name);
                                
                                if ui.small_button("✖").clicked() {
                                    self.input_paths.remove(i);
                                }
                            });
                        }
                    });
                }
                
                // Analyze button
                if ui.button("Run Analysis").clicked() && !self.is_analyzing && !self.keywords.is_empty() && !self.input_paths.is_empty() {
                    self.perform_analysis();
                }
            });
            
            ui.separator();
            
            // Results section
            if let Some(results) = &self.results {
                ui.heading("Analysis Results");
                
                ui.label(format!("Analyzed {} documents with {} keywords", 
                    results.total_documents, results.keywords.len()));
                ui.label(format!("Correlation threshold: {:.2}", self.correlation_threshold));
                
                ui.separator();
                
                // Correlation matrix
                ui.collapsing("Keyword Correlation Matrix", |ui| {
                    if results.keywords.len() <= 1 {
                        ui.label("Need at least 2 keywords to show correlations");
                    } else {
                        // Draw correlation matrix
                        let _matrix_size = Vec2::new(
                            (results.keywords.len() * 60) as f32, 
                            (results.keywords.len() * 30) as f32
                        );
                        
                        egui::ScrollArea::both().max_height(300.0).show(ui, |ui| {
                            // Matrix header row
                            ui.horizontal(|ui| {
                                ui.add_space(60.0); // For the row headers
                                
                                for keyword in &results.keywords {
                                    ui.label(RichText::new(keyword).strong());
                                }
                            });
                            
                            // Matrix data rows
                            for (i, row_keyword) in results.keywords.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(row_keyword).strong());
                                    
                                    for (j, _) in results.keywords.iter().enumerate() {
                                        let value = if i == j {
                                            String::from("—")
                                        } else if i < j {
                                            format!("{:.2}", results.correlation_matrix[i][j])
                                        } else {
                                            format!("{:.2}", results.correlation_matrix[j][i])
                                        };
                                        
                                        ui.label(value);
                                    }
                                });
                            }
                        });
                    }
                });
                
                ui.separator();
                
                // Ranked documents
                ui.collapsing("Ranked Documents", |ui| {
                    if results.ranked_docs.is_empty() {
                        ui.label("No documents matched the analysis criteria");
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (i, doc) in results.ranked_docs.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}. ", i+1));
                                    ui.label(RichText::new(&doc.name).strong());
                                    ui.label(format!("(score: {:.2})", doc.score));
                                    
                                    if ui.button("Open").clicked() {
                                        pdf_viewer.load_pdf(&doc.path);
                                    }
                                });
                            }
                        });
                    }
                });
            } else if self.is_analyzing {
                ui.label("Analyzing documents...");
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.label("No analysis results yet");
                    ui.label("Configure the analysis parameters and click Run Analysis");
                });
            }
        });
    }

    fn show_heatmap(&mut self, _ui: &mut Ui, _data: &[f32]) {
        let _matrix_size = Vec2::new(100.0, 100.0);
        // ... existing code (if any) ...
    }
} 