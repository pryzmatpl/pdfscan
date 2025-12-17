use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use egui::{Context, Ui, RichText, Color32, TextEdit, Key};

use super::pdf_viewer::PdfViewer;

/// Search panel component
pub struct SearchPanel {
    search_query: String,
    search_results: Vec<SearchResult>,
    search_paths: Vec<PathBuf>,
    case_sensitive: bool,
    search_scope: SearchScope,
    directory_path: Option<PathBuf>,
    is_searching: bool,
    create_zip: bool,
    loaded_pdfs: Vec<PathBuf>,
    pdf_cache: HashMap<PathBuf, String>,
    is_loading_directory: bool,
    directory_loading_progress: Option<(usize, usize)>,
    directory_filter: String,
}

/// Search result
struct SearchResult {
    file_path: PathBuf,
    file_name: String,
    match_count: usize,
    matches: Vec<MatchResult>,
}

/// Match within a file
struct MatchResult {
    text: String,
    position: usize,
}

#[derive(PartialEq)]
enum SearchScope {
    CurrentDocument,
    Directory,
}

impl SearchPanel {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            search_results: Vec::new(),
            search_paths: Vec::new(),
            case_sensitive: false,
            search_scope: SearchScope::CurrentDocument,
            directory_path: None,
            is_searching: false,
            create_zip: false,
            loaded_pdfs: Vec::new(),
            pdf_cache: HashMap::new(),
            is_loading_directory: false,
            directory_loading_progress: None,
            directory_filter: String::new(),
        }
    }
    
    /// Show search options in the sidebar
    pub fn show_options(&mut self, ui: &mut Ui, pdf_viewer: &PdfViewer) {
        ui.heading("Search Options");
        
        // Search query
        ui.label("Search for:");
        let text_edit = TextEdit::singleline(&mut self.search_query)
            .hint_text("Enter search text...")
            .desired_width(ui.available_width());
        
        ui.add(text_edit);
        
        ui.add_space(5.0);
        
        // Search options
        ui.checkbox(&mut self.case_sensitive, "Case sensitive");
        
        ui.add_space(10.0);
        
        // Search scope
        ui.label(RichText::new("Search scope:").strong());
        
        let has_current = pdf_viewer.current_pdf().is_some();
        
        // Properly use radio buttons with an enum
        let current_doc_response = ui.radio(self.search_scope == SearchScope::CurrentDocument, "Current document");
        if current_doc_response.clicked() {
            self.search_scope = SearchScope::CurrentDocument;
        }
        
        if !has_current {
            ui.horizontal(|ui| {
                ui.label(RichText::new("(No document open)").italics().color(Color32::GRAY));
            });
        }
        
        let dir_response = ui.radio(self.search_scope == SearchScope::Directory, "Directory");
        if dir_response.clicked() {
            self.search_scope = SearchScope::Directory;
        }
        
        if self.search_scope == SearchScope::Directory {
            ui.horizontal(|ui| {
                ui.label("   ");  // Indent
                if ui.button("📁 Select...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.directory_path = Some(path.clone());
                        self.load_directory_pdfs(&path);
                    }
                }
            });
            
            // Show selected directory
            if let Some(path) = &self.directory_path {
                ui.group(|ui| {
                    ui.label(RichText::new("Selected directory:").strong());
                    ui.label(path.to_string_lossy().to_string());
                    ui.label(format!("{} PDF files found", self.loaded_pdfs.len()));
                });
            } else {
                ui.label(RichText::new("No directory selected").italics());
            }
            
            if self.is_loading_directory {
                if let Some((current, total)) = self.directory_loading_progress {
                    ui.label(format!("Loading PDFs: {}/{}", current, total));
                } else {
                    ui.label("Loading PDFs...");
                }
            }
            
            ui.checkbox(&mut self.create_zip, "Create ZIP with results");
        }
        
        ui.add_space(15.0);
        
        // Search button
        let button_enabled = (!self.search_query.is_empty()) && 
                          ((self.search_scope == SearchScope::CurrentDocument && has_current) || 
                           (self.search_scope == SearchScope::Directory && self.directory_path.is_some()));
        
        if ui.add_enabled(button_enabled && !self.is_searching,
            egui::Button::new(if self.is_searching { "Searching..." } else { "Search" })
                .min_size(egui::vec2(120.0, 28.0))  // Make button more prominent
                .fill(ui.style().visuals.selection.bg_fill))
            .clicked()
        {
            self.perform_search(pdf_viewer);
        }
    }
    
    /// Load directory (public method)
    pub fn load_directory(&mut self, dir_path: &PathBuf) {
        self.directory_path = Some(dir_path.clone());
        self.load_directory_pdfs(dir_path);
    }
    
    /// Load all PDFs from a directory
    fn load_directory_pdfs(&mut self, dir_path: &PathBuf) {
        self.is_loading_directory = true;
        self.loaded_pdfs.clear();
        self.pdf_cache.clear();
        self.directory_loading_progress = Some((0, 0));
        
        // First, quickly scan for PDF files synchronously
        let mut pdfs = Vec::new();
        for entry in walkdir::WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.to_str() == Some("pdf") {
                        pdfs.push(path.to_path_buf());
                    }
                }
            }
        }
        
        self.loaded_pdfs = pdfs.clone();
        self.directory_loading_progress = Some((0, self.loaded_pdfs.len()));
        
        // Now extract text in background
        if !pdfs.is_empty() {
            let pdf_cache_arc = Arc::new(Mutex::new(HashMap::<PathBuf, String>::new()));
            let pdf_cache_clone = pdf_cache_arc.clone();
            let progress_arc = Arc::new(Mutex::new(None::<(usize, usize)>));
            let progress_clone = progress_arc.clone();
            let is_loading_arc = Arc::new(Mutex::new(true));
            let is_loading_clone = is_loading_arc.clone();
            
            // Create cache directory
            let cache_dir = get_cache_directory(&dir_path);
            let cache_dir_clone = cache_dir.clone();
            
            // Load PDFs with thread pool (1 thread per 20 files)
            const FILES_PER_THREAD: usize = 20;
            let num_threads = (pdfs.len() + FILES_PER_THREAD - 1) / FILES_PER_THREAD;
            
            let pdfs_arc = Arc::new(pdfs);
            let mut handles = Vec::new();
            
            for thread_idx in 0..num_threads {
                let pdfs_clone = pdfs_arc.clone();
                let cache_dir_thread = cache_dir_clone.clone();
                let progress_clone_thread = progress_clone.clone();
                let pdf_cache_clone_thread = pdf_cache_clone.clone();
                let is_loading_clone_thread = is_loading_clone.clone();
                
                let handle = std::thread::spawn(move || {
                    let start_idx = thread_idx * FILES_PER_THREAD;
                    let end_idx = (start_idx + FILES_PER_THREAD).min(pdfs_clone.len());
                    
                    let mut local_cache = HashMap::new();
                    
                    // Process files assigned to this thread
                    for idx in start_idx..end_idx {
                        let pdf_path = &pdfs_clone[idx];
                        
                        // Check cache first
                        let cache_path = get_cache_path(pdf_path, &cache_dir_thread);
                        let text = if let Some(cached_text) = load_text_from_cache(&cache_path) {
                            // Use cached text
                            cached_text
                        } else {
                            // Extract text and save to cache
                            match extract_text_from_pdf_safe(pdf_path) {
                                Ok(extracted_text) => {
                                    // Save to cache (ignore errors)
                                    let _ = save_text_to_cache(&cache_path, &extracted_text);
                                    extracted_text
                                },
                                Err(e) => {
                                    eprintln!("Failed to extract text from {}: {}", pdf_path.display(), e);
                                    String::new()
                                }
                            }
                        };
                        
                        if !text.is_empty() {
                            local_cache.insert(pdf_path.clone(), text);
                        }
                        
                        // Update progress
                        {
                            let mut prog = progress_clone_thread.lock().unwrap();
                            *prog = Some((idx + 1, pdfs_clone.len()));
                        }
                    }
                    
                    // Merge local cache into shared cache
                    {
                        let mut cache_guard = pdf_cache_clone_thread.lock().unwrap();
                        cache_guard.extend(local_cache);
                    }
                });
                
                handles.push(handle);
            }
            
            // Spawn a thread to wait for all workers and mark completion
            std::thread::spawn(move || {
                for handle in handles {
                    let _ = handle.join();
                }
                
                // Mark loading as complete
                {
                    let mut loading = is_loading_clone.lock().unwrap();
                    *loading = false;
                }
            });
            
            // Store Arc reference for later access (we'll need to check completion)
            // For now, we'll extract text lazily during search
        } else {
            self.is_loading_directory = false;
        }
    }
    
    /// Check for loaded directory PDFs and update state
    fn update_directory_loading(&mut self, ctx: &Context) {
        if self.is_loading_directory {
            // In a real implementation, we'd use channels or async
            // For now, we'll mark as complete after a short delay
            // The background thread will update the cache as it processes files
            ctx.request_repaint();
            
            // Check if we should mark loading as complete
            // (In production, check thread completion status)
            if let Some((current, total)) = self.directory_loading_progress {
                if current >= total && total > 0 {
                    self.is_loading_directory = false;
                    self.directory_loading_progress = None;
                }
            }
        }
    }
    
    /// Perform a search operation
    fn perform_search(&mut self, pdf_viewer: &PdfViewer) {
        // Validate search query
        if self.search_query.trim().is_empty() {
            self.is_searching = false;
            return;
        }
        
        self.is_searching = true;
        self.search_results.clear();
        
        // Search in current document
        if self.search_scope == SearchScope::CurrentDocument {
            if let Some(pdf_path) = pdf_viewer.current_pdf() {
                let text = pdf_viewer.text();
                let matches = self.search_in_text(&text);
                
                if !matches.is_empty() {
                    let result = SearchResult {
                        file_path: pdf_path.clone(),
                        file_name: pdf_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                        match_count: matches.len(),
                        matches,
                    };
                    
                    self.search_results.push(result);
                }
            }
        }
        // Search in directory
        else if self.search_scope == SearchScope::Directory {
            if let Some(_dir_path) = &self.directory_path {
                // Use loaded PDFs and cache for searching
                let search_query = self.search_query.clone();
                let case_sensitive = self.case_sensitive;
                let loaded_pdfs = self.loaded_pdfs.clone();
                let pdf_cache = self.pdf_cache.clone();
                
                // Search through loaded PDFs
                let mut results = Vec::new();
                
                // Search through loaded PDFs
                for pdf_path in &loaded_pdfs {
                    // Get text from cache or extract it
                    let text = if let Some(cached_text) = pdf_cache.get(pdf_path) {
                        cached_text.clone()
                    } else {
                        // Try to load from cache file first
                        let cache_dir = if let Some(dir) = &self.directory_path {
                            get_cache_directory(dir)
                        } else {
                            pdf_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
                        };
                        let cache_path = get_cache_path(pdf_path, &cache_dir);
                        
                        let extracted_text = if let Some(cached_text) = load_text_from_cache(&cache_path) {
                            // Use cached text
                            cached_text
                        } else {
                            // Extract text safely and save to cache
                            match extract_text_from_pdf_safe(pdf_path) {
                                Ok(text) => {
                                    // Save to cache (ignore errors)
                                    let _ = save_text_to_cache(&cache_path, &text);
                                    text
                                },
                                Err(_) => {
                                    continue; // Skip this PDF if extraction fails
                                }
                            }
                        };
                        
                        // Cache in memory for this session
                        self.pdf_cache.insert(pdf_path.clone(), extracted_text.clone());
                        extracted_text
                    };
                    
                    // Search in text
                    let search_text = if case_sensitive {
                        text.clone()
                    } else {
                        text.to_lowercase()
                    };
                    
                    let query_lower = if case_sensitive {
                        search_query.clone()
                    } else {
                        search_query.to_lowercase()
                    };
                    
                    let matches = search_text.matches(&query_lower).count();
                    
                    if matches > 0 {
                        let file_name = pdf_path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        
                        // Find match positions for context
                        let match_results = self.search_in_text(&text);
                        
                        results.push(SearchResult {
                            file_path: pdf_path.clone(),
                            file_name,
                            match_count: matches,
                            matches: match_results,
                        });
                    }
                }
                
                self.search_results = results;
                
                // Create ZIP file if requested
                if self.create_zip && !self.search_results.is_empty() {
                    self.create_zip_with_results();
                }
            }
        }
        
        self.is_searching = false;
    }
    
    /// Create a ZIP file with search results
    fn create_zip_with_results(&self) {
        if self.search_results.is_empty() {
            return;
        }
        
        // Get paths to include in the ZIP
        let pdf_paths: Vec<String> = self.search_results.iter()
            .map(|r| r.file_path.to_string_lossy().to_string())
            .collect();
            
        // Use the zip function from the search module
        let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
        let zip_file_name = format!("search_results_{}.zip", timestamp);
        
        if let Err(e) = crate::search::zip_files(&zip_file_name, &pdf_paths) {
            eprintln!("Error creating ZIP file: {}", e);
        } else {
            println!("Created ZIP file with search results: {}", zip_file_name);
        }
    }
    
    /// Search for matches in text
    /// This method safely handles UTF-8 characters and prevents crashes
    fn search_in_text(&self, text: &str) -> Vec<MatchResult> {
        let mut matches = Vec::new();
        
        // Return early if query is empty
        if self.search_query.is_empty() {
            return matches;
        }
        
        let query = if self.case_sensitive {
            self.search_query.clone()
        } else {
            self.search_query.to_lowercase()
        };
        
        // If query is empty after processing, return
        if query.is_empty() {
            return matches;
        }
        
        let search_text = if self.case_sensitive {
            text.to_string()
        } else {
            text.to_lowercase()
        };
        
        // Use char_indices to handle UTF-8 properly
        let text_chars: Vec<(usize, char)> = text.char_indices().collect();
        let search_chars: Vec<char> = search_text.chars().collect();
        let query_chars: Vec<char> = query.chars().collect();
        
        if query_chars.is_empty() || search_chars.len() < query_chars.len() {
            return matches;
        }
        
        // Find all occurrences using character matching
        let mut start_char_idx = 0;
        while start_char_idx <= search_chars.len().saturating_sub(query_chars.len()) {
            // Check if we have a match starting at start_char_idx
            let mut matched = true;
            for (i, &qc) in query_chars.iter().enumerate() {
                if start_char_idx + i >= search_chars.len() || search_chars[start_char_idx + i] != qc {
                    matched = false;
                    break;
                }
            }
            
            if matched {
                // Found a match at start_char_idx
                // Get the byte position in the original text
                let byte_pos = if start_char_idx < text_chars.len() {
                    text_chars[start_char_idx].0
                } else {
                    text.len()
                };
                
                // Extract context (40 chars before and after)
                let context_before = 40;
                let context_after = 40;
                
                let context_start_char = start_char_idx.saturating_sub(context_before);
                let context_end_char = (start_char_idx + query_chars.len() + context_after)
                    .min(text_chars.len());
                
                // Get byte positions for context
                let context_start_byte = if context_start_char < text_chars.len() {
                    text_chars[context_start_char].0
                } else {
                    0
                };
                
                let context_end_byte = if context_end_char < text_chars.len() {
                    text_chars[context_end_char].0
                } else {
                    text.len()
                };
                
                // Safely extract context
                let context = if context_start_byte < context_end_byte && context_end_byte <= text.len() {
                    text.get(context_start_byte..context_end_byte)
                        .unwrap_or("")
                        .to_string()
                } else {
                    String::new()
                };
                
                matches.push(MatchResult {
                    text: context,
                    position: byte_pos,
                });
                
                // Move past this match
                start_char_idx += query_chars.len();
            } else {
                start_char_idx += 1;
            }
        }
        
        matches
    }
    
    /// Show the search panel in the main content area
    pub fn show(&mut self, ui: &mut Ui, ctx: &Context, pdf_viewer: &mut PdfViewer) {
        // Update directory loading status
        self.update_directory_loading(ctx);
        ui.vertical(|ui| {
            // Top search bar
            ui.horizontal(|ui| {
                ui.heading("PDF Search");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📁 Select Directory").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.directory_path = Some(path.clone());
                            self.search_scope = SearchScope::Directory;
                            self.load_directory_pdfs(&path);
                        }
                    }
                });
            });
            
            ui.separator();
            
            // Split view for directory tree and results
            egui::TopBottomPanel::top("search_top_panel")
                .resizable(true)
                .height_range(50.0..=150.0)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Search input with Enter key handling
                        let text_edit_response = TextEdit::singleline(&mut self.search_query)
                            .hint_text("Search in PDFs...")
                            .desired_width(ui.available_width() - 20.0)
                            .show(ui);
                        
                        ui.checkbox(&mut self.case_sensitive, "Case sensitive");
                        
                        let button_enabled = !self.search_query.is_empty() && 
                            ((self.search_scope == SearchScope::CurrentDocument && pdf_viewer.current_pdf().is_some()) || 
                            (self.search_scope == SearchScope::Directory && self.directory_path.is_some()));
                        
                        // Check for Enter key press to trigger search
                        if button_enabled && !self.is_searching && 
                           (text_edit_response.response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                            self.perform_search(pdf_viewer);
                        }
                    });
                    
                    // Search scope selection
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.search_scope, SearchScope::CurrentDocument, "Current document");
                        ui.radio_value(&mut self.search_scope, SearchScope::Directory, "Directory");
                        
                        if self.search_scope == SearchScope::Directory {
                            if let Some(dir) = &self.directory_path {
                                ui.label(dir.to_string_lossy().to_string());
                            } else {
                                ui.label(RichText::new("No directory selected").italics());
                            }
                            
                            ui.checkbox(&mut self.create_zip, "Create ZIP");
                        }
                    });
                });
            
            // Show directory PDFs list or search results
            egui::CentralPanel::default().show_inside(ui, |ui| {
                if self.search_results.is_empty() && self.search_scope == SearchScope::Directory && !self.loaded_pdfs.is_empty() {
                    // Show directory PDFs list
                    self.show_directory_pdfs(ui, pdf_viewer);
                } else {
                    // Show search results
                    self.show_results(ui, pdf_viewer, ctx);
                }
            });
        });
    }
    
    /// Show directory PDFs list
    fn show_directory_pdfs(&mut self, ui: &mut Ui, pdf_viewer: &mut PdfViewer) {
        ui.horizontal(|ui| {
            ui.heading(format!("PDF Files in Directory ({} files)", self.loaded_pdfs.len()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 Reload").clicked() {
                    if let Some(dir_path) = self.directory_path.clone() {
                        self.load_directory_pdfs(&dir_path);
                    }
                }
            });
        });
        
        ui.separator();
        
        // Filter input
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.add(TextEdit::singleline(&mut self.directory_filter)
                .hint_text("Filter by filename...")
                .desired_width(200.0));
            
            if !self.directory_filter.is_empty() {
                if ui.button("✖").clicked() {
                    self.directory_filter.clear();
                }
            }
        });
        
        ui.separator();
        
        // Filter PDFs based on filter string
        let filtered_pdfs: Vec<&PathBuf> = if self.directory_filter.is_empty() {
            self.loaded_pdfs.iter().collect()
        } else {
            let filter_lower = self.directory_filter.to_lowercase();
            self.loaded_pdfs.iter()
                .filter(|path| {
                    let file_name = path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase();
                    file_name.contains(&filter_lower)
                })
                .collect()
        };
        
        ui.label(format!("Showing {} of {} files", filtered_pdfs.len(), self.loaded_pdfs.len()));
        
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for (idx, pdf_path) in filtered_pdfs.iter().enumerate() {
                    let file_name = pdf_path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    
                    ui.horizontal(|ui| {
                        ui.label(format!("{}. ", idx + 1));
                        
                        if ui.button("📄 Open").clicked() {
                            pdf_viewer.load_pdf(pdf_path);
                        }
                        
                        // Limit filename length
                        let display_name = if file_name.len() > 60 {
                            format!("{}...", &file_name[..60])
                        } else {
                            file_name
                        };
                        ui.label(egui::RichText::new(display_name).strong());
                        
                        // Show if text is cached
                        if self.pdf_cache.contains_key(*pdf_path) {
                            ui.label(RichText::new("✓").color(Color32::GREEN).small());
                        }
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Truncate long paths
                            let path_str = pdf_path.to_string_lossy();
                            let display_path = if path_str.len() > 80 {
                                format!("...{}", &path_str[path_str.len().saturating_sub(80)..])
                            } else {
                                path_str.to_string()
                            };
                            ui.label(egui::RichText::new(display_path).small().weak());
                        });
                    });
                    
                    ui.separator();
                }
            });
    }
    
    /// Show the search results
    fn show_results(&mut self, ui: &mut Ui, pdf_viewer: &mut PdfViewer, ctx: &Context) {
        // Store search query in memory for highlighting
        if !self.search_query.is_empty() {
            ui.memory_mut(|mem| mem.data.insert_temp("search_query".into(), self.search_query.clone()));
        }
        
        if self.is_searching {
            ui.spinner();
            ui.label("Searching...");
            return;
        }
        
        // Show results count
        ui.horizontal(|ui| {
            if self.search_results.is_empty() {
                ui.label(RichText::new("No results found").italics());
            } else {
                let total_matches: usize = self.search_results.iter().map(|r| r.match_count).sum();
                ui.label(RichText::new(format!("{} matches found in {} file(s)", total_matches, self.search_results.len())).strong());
            }
        });
        
        if self.search_results.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                
                if !self.search_query.is_empty() {
                    ui.label("Try different search terms or checking a different location");
                } else {
                    ui.label("Enter a search term and press Enter to search");
                }
                
                ui.add_space(20.0);
            });
        } else {
            // Limit number of results displayed to prevent UI issues
            let max_results_to_show = 100;
            let results_to_show: Vec<&SearchResult> = self.search_results.iter().take(max_results_to_show).collect();
            
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    if self.search_results.len() > max_results_to_show {
                        ui.label(format!("Showing first {} of {} results", max_results_to_show, self.search_results.len()));
                        ui.separator();
                    }
                    
                    for result in results_to_show {
                        // Format header with file name and match count
                        let header = format!(
                            "{} ({} {})", 
                            result.file_name, 
                            result.match_count,
                            if result.match_count == 1 { "match" } else { "matches" }
                        );
                        
                        egui::CollapsingHeader::new(header)
                            .id_source(&result.file_path)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if ui.button("Open PDF").clicked() {
                                        pdf_viewer.load_pdf(&result.file_path);
                                    }
                                    // Truncate long paths to prevent rendering issues
                                    let path_str = result.file_path.to_string_lossy();
                                    let display_path = if path_str.len() > 100 {
                                        format!("...{}", &path_str[path_str.len().saturating_sub(100)..])
                                    } else {
                                        path_str.to_string()
                                    };
                                    ui.label(egui::RichText::new(display_path).monospace());
                                });
                                
                                ui.add_space(5.0);
                                
                                // Show matches (limit to prevent UI issues)
                                let max_matches_to_show = 50;
                                for (i, m) in result.matches.iter().take(max_matches_to_show).enumerate() {
                                    ui.group(|ui| {
                                        // Limit text length to prevent rendering issues
                                        const MAX_TEXT_LENGTH: usize = 200;
                                        let display_text = if m.text.len() > MAX_TEXT_LENGTH {
                                            format!("{}...", &m.text[..MAX_TEXT_LENGTH.min(m.text.len())])
                                        } else {
                                            m.text.clone()
                                        };
                                        
                                        // Use simple label instead of complex RichText formatting
                                        // to avoid font rendering issues
                                        ui.label(format!("{}. {}", i + 1, display_text));
                                        
                                        if ui.button("Jump to match").clicked() {
                                            // Calculate the approximate page number based on position
                                            let text = pdf_viewer.text();
                                            if !text.is_empty() {
                                                let position_ratio = m.position as f32 / text.len() as f32;
                                                let page = (position_ratio * pdf_viewer.total_pages() as f32).floor() as usize;
                                                
                                                // Jump to the calculated page with search term highlighting
                                                pdf_viewer.jump_to_page(page, Some(&self.search_query), ctx);
                                            } else {
                                                // Just load the PDF if we can't calculate the page
                                                pdf_viewer.load_pdf(&result.file_path);
                                            }
                                        }
                                    });
                                }
                                
                                // Show message if there are more matches
                                if result.matches.len() > max_matches_to_show {
                                    ui.label(format!("... and {} more matches", result.matches.len() - max_matches_to_show));
                                }
                            });
                    }
                });
        }
    }
}

/// Search for PDF files containing the given phrase in a directory
fn search_files_in_directory(dir: &PathBuf, search_phrase: &str) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();
    
    // Walk through all files in the directory
    for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == "pdf" {
                    // Check if PDF contains the search phrase
                    match search_phrase_in_pdf(path, search_phrase) {
                        Ok(true) => {
                            results.push(path.to_path_buf());
                        },
                        Ok(false) => {}, // Phrase not found
                        Err(e) => eprintln!("Error processing {}: {}", path.display(), e),
                    }
                }
            }
        }
    }
    
    Ok(results)
}

/// Check if a PDF file contains the search phrase (safely)
fn search_phrase_in_pdf(file_path: &Path, search_phrase: &str) -> Result<bool, Box<dyn std::error::Error>> {
    // Try cache first
    let cache_dir = file_path.parent().unwrap_or_else(|| Path::new("."));
    let cache_path = get_cache_path(file_path, cache_dir);
    
    let text = if let Some(cached_text) = load_text_from_cache(&cache_path) {
        cached_text
    } else {
        // Extract text safely
        match extract_text_from_pdf_safe(file_path) {
            Ok(extracted_text) => {
                // Save to cache
                let _ = save_text_to_cache(&cache_path, &extracted_text);
                extracted_text
            },
            Err(e) => {
                return Err(e); // Return error if extraction fails
            }
        }
    };
    
    Ok(text.contains(search_phrase))
}

/// Extract text from a PDF file safely, handling panics
fn extract_text_from_pdf_safe(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    
    // Wrap in panic handler to prevent crashes from malformed PDFs
    let text = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(&bytes)
    }));
    
    match text {
        Ok(Ok(text)) => Ok(text),
        Ok(Err(e)) => {
            eprintln!("Error extracting text from {}: {}", path.display(), e);
            Ok(String::new()) // Return empty string on error, don't fail completely
        },
        Err(_) => {
            // Panic occurred (likely from type1-encoding-parser with malformed fonts)
            eprintln!("Warning: PDF text extraction panicked for {} (malformed PDF?)", path.display());
            Ok(String::new()) // Return empty string rather than crashing
        }
    }
}

/// Get cache file path for a PDF
fn get_cache_path(pdf_path: &Path, cache_dir: &Path) -> PathBuf {
    let pdf_name = pdf_path.file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    cache_dir.join(format!("{}.txt", pdf_name))
}

/// Get or create .pdfscan cache directory
fn get_cache_directory(pdf_dir: &Path) -> PathBuf {
    pdf_dir.join(".pdfscan")
}

/// Load text from cache file if it exists
fn load_text_from_cache(cache_path: &Path) -> Option<String> {
    std::fs::read_to_string(cache_path).ok()
}

/// Save text to cache file
fn save_text_to_cache(cache_path: &Path, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Create parent directory if it doesn't exist
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(cache_path, text)?;
    Ok(())
}
