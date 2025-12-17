use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::process::Command;
use std::io::Read;
use egui::{Context, Ui, Vec2, RichText, Color32, TextureHandle};
use lopdf::Document;
use image::{ImageBuffer, Rgba, DynamicImage};

/// PDF viewer component that displays PDFs with rendering support
pub struct PdfViewer {
    current_pdf_path: Option<PathBuf>,
    document: Option<Arc<Document>>,
    current_page: usize,
    total_pages: usize,
    pages: HashMap<usize, PageData>,
    page_textures: HashMap<usize, TextureHandle>,
    document_title: String,
    outline: Vec<OutlineItem>,
    text_data: Arc<Mutex<String>>,
    loading: bool,
    document_loaded: Arc<Mutex<Option<Arc<Document>>>>,
    show_text_panel: bool,
    zoom: f32,
    rendering_pages: Arc<Mutex<Vec<usize>>>, // Pages currently being rendered
    use_poppler: bool, // Whether poppler is available
    rendered_images: Arc<Mutex<HashMap<usize, (Vec<u8>, (u32, u32))>>>, // Rendered images waiting to be loaded as textures
}

/// Page data
struct PageData {
    text: String,
    size: Vec2,
}

/// Outline item
struct OutlineItem {
    title: String,
    page: usize,
    level: usize,
    children: Vec<OutlineItem>,
}

impl PdfViewer {
    pub fn new() -> Self {
        // Check if pdftoppm is available
        let use_poppler = Command::new("pdftoppm")
            .arg("-v")
            .output()
            .is_ok();
        
        if !use_poppler {
            eprintln!("Warning: pdftoppm not found. PDF rendering will be limited to text-only.");
            eprintln!("Install poppler-utils for full PDF rendering: sudo pacman -S poppler");
        }
        
        Self {
            current_pdf_path: None,
            document: None,
            current_page: 0,
            total_pages: 0,
            pages: HashMap::new(),
            page_textures: HashMap::new(),
            document_title: String::new(),
            outline: Vec::new(),
            text_data: Arc::new(Mutex::new(String::new())),
            loading: false,
            document_loaded: Arc::new(Mutex::new(None)),
            show_text_panel: false,
            zoom: 1.0,
            rendering_pages: Arc::new(Mutex::new(Vec::new())),
            use_poppler,
            rendered_images: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Load a PDF file
    pub fn load_pdf(&mut self, path: &Path) {
        self.loading = true;
        self.current_pdf_path = Some(path.to_path_buf());
        
        // Create a clone for the async task
        let path_clone = path.to_path_buf();
        let text_data = self.text_data.clone();
        let document_loaded = self.document_loaded.clone();
        
        // Reset state
        self.document = None;
        self.current_page = 0;
        self.total_pages = 0;
        self.pages.clear();
        self.page_textures.clear();
        self.document_title = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        
        // Load the PDF in a separate thread
        std::thread::spawn(move || {
            // Load with lopdf for structure parsing (optional, for compatibility)
            let lopdf_result = Document::load(&path_clone);
            
            // Extract text for search and analysis
            match extract_text_from_pdf(&path_clone) {
                Ok(text) => {
                    let mut text_data = text_data.lock().unwrap();
                    *text_data = text;
                },
                Err(e) => {
                    eprintln!("Error extracting text: {}", e);
                }
            }
            
            if let Ok(document) = lopdf_result {
                // Store the loaded document in the shared mutex
                let doc = Arc::new(document);
                let mut document_loaded = document_loaded.lock().unwrap();
                *document_loaded = Some(doc);
            } else {
                eprintln!("Error loading PDF with lopdf (optional)");
            }
        });
    }
    
    /// Process loaded document (should be called from the UI thread)
    fn process_loaded_document(&mut self, ctx: &Context) {
        if self.loading {
            // Check if document has been loaded by the background thread
            let doc_option = {
                let mut document_loaded = self.document_loaded.lock().unwrap();
                document_loaded.take()
            };

            if let Some(doc) = doc_option {
                // Update state with the loaded document
                self.document = Some(doc.clone());
                
                // Get page count from lopdf
                self.total_pages = doc.get_pages().len();
                
                // Render first page if poppler is available
                if self.use_poppler && self.total_pages > 0 {
                    self.render_page(0, ctx);
                }
                
                // Load first page text
                self.extract_page_text(0);
            }
            
            // If we got here and still loading, check if we have at least text data
            if self.loading {
                let has_text = {
                    let text_data = self.text_data.lock().unwrap();
                    !text_data.is_empty()
                };
                
                // If we have text but no page count, estimate
                if has_text && self.total_pages == 0 {
                    // Estimate pages from text length (rough estimate: ~2000 chars per page)
                    let text_len = {
                        let text_data = self.text_data.lock().unwrap();
                        text_data.len()
                    };
                    self.total_pages = (text_len / 2000).max(1);
                }
                
                // Mark as not loading if we have some data
                if self.total_pages > 0 || has_text {
                    self.loading = false;
                }
            }
        }
    }
    
    /// Render a PDF page using pdftoppm
    fn render_page(&mut self, page_num: usize, ctx: &Context) {
        // Check if already rendered
        if self.page_textures.contains_key(&page_num) {
            return;
        }
        
        // Check if already rendering
        {
            let mut rendering = self.rendering_pages.lock().unwrap();
            if rendering.contains(&page_num) {
                return; // Already rendering
            }
            rendering.push(page_num);
        }
        
        if !self.use_poppler {
            // Fallback to text-only
            self.extract_page_text(page_num);
            let mut rendering = self.rendering_pages.lock().unwrap();
            rendering.retain(|&x| x != page_num);
            return;
        }
        
        let pdf_path = match &self.current_pdf_path {
            Some(p) => p.clone(),
            None => {
                let mut rendering = self.rendering_pages.lock().unwrap();
                rendering.retain(|&x| x != page_num);
                return;
            }
        };
        
        let page_num_clone = page_num;
        let ctx_clone = ctx.clone();
        let rendered_images_clone = self.rendered_images.clone();
        let rendering_pages_clone = self.rendering_pages.clone();
        
        // Render in background thread
        std::thread::spawn(move || {
            // Use pdftoppm to render the page
            let dpi = 150; // Good quality
            let output = Command::new("pdftoppm")
                .arg("-png")
                .arg("-r")
                .arg(dpi.to_string())
                .arg("-f")
                .arg((page_num_clone + 1).to_string())
                .arg("-l")
                .arg((page_num_clone + 1).to_string())
                .arg(&pdf_path)
                .arg("-")
                .output();
            
            match output {
                Ok(output) if output.status.success() => {
                    // Parse PNG from stdout
                    match image::load_from_memory(&output.stdout) {
                        Ok(img) => {
                            let rgba = img.to_rgba8();
                            let width = rgba.width();
                            let height = rgba.height();
                            let pixels = rgba.into_raw();
                            
                            // Store rendered image for main thread to load as texture
                            let mut rendered = rendered_images_clone.lock().unwrap();
                            rendered.insert(page_num_clone, (pixels, (width, height)));
                            
                            // Request repaint to load texture
                            ctx_clone.request_repaint();
                        },
                        Err(e) => {
                            eprintln!("Failed to parse PNG from pdftoppm: {}", e);
                        }
                    }
                },
                Ok(output) => {
                    eprintln!("pdftoppm failed: {}", String::from_utf8_lossy(&output.stderr));
                },
                Err(e) => {
                    eprintln!("Failed to run pdftoppm: {}", e);
                }
            }
            
            // Remove from rendering list
            let mut rendering = rendering_pages_clone.lock().unwrap();
            rendering.retain(|&x| x != page_num_clone);
        });
    }
    
    /// Load rendered images as textures (called from main thread)
    fn load_rendered_textures(&mut self, ctx: &Context) {
        let mut rendered = self.rendered_images.lock().unwrap();
        let to_load: Vec<(usize, Vec<u8>, (u32, u32))> = rendered.iter()
            .map(|(k, v)| (*k, v.0.clone(), v.1))
            .collect();
        rendered.clear();
        drop(rendered);
        
        for (page_num, pixels, (width, height)) in to_load {
            let size = [width as usize, height as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
            let texture = ctx.load_texture(
                format!("pdf_page_{}", page_num),
                color_image,
                egui::TextureOptions::default()
            );
            self.page_textures.insert(page_num, texture);
            
            // Also update page data with size
            if let Some(page_data) = self.pages.get_mut(&page_num) {
                page_data.size = Vec2::new(width as f32, height as f32);
            } else {
                self.pages.insert(page_num, PageData {
                    text: String::new(),
                    size: Vec2::new(width as f32, height as f32),
                });
            }
        }
    }
    
    /// Extract text from a specific page
    fn extract_page_text(&mut self, page_num: usize) {
        if self.pages.contains_key(&page_num) {
            return; // Already loaded
        }
        
        // Load page text from extracted text data
        self.load_page_text(page_num);
    }
    
    /// Load page text content from extracted text
    fn load_page_text(&mut self, page_num: usize) {
        // If we already have page data, skip
        if self.pages.contains_key(&page_num) {
            return;
        }
        
        // Default page size
        let size = Vec2::new(612.0, 792.0); // Letter size
        
        // Get text content (from the already extracted text)
        let text = {
            let text_data = self.text_data.lock().unwrap();
            text_data.clone()
        };
        
        // Split text by pages (rough estimate)
        let lines: Vec<&str> = text.lines().collect();
        let lines_per_page = lines.len().max(1) / self.total_pages.max(1);
        let start_line = page_num * lines_per_page;
        let end_line = ((page_num + 1) * lines_per_page).min(lines.len());
        
        let page_text = if start_line < lines.len() {
            lines[start_line..end_line].join("\n")
        } else {
            format!("Page {} content", page_num + 1)
        };
        
        self.pages.insert(page_num, PageData { 
            text: page_text,
            size 
        });
    }
    
    /// Get the current PDF path
    pub fn current_pdf(&self) -> Option<&PathBuf> {
        self.current_pdf_path.as_ref()
    }
    
    /// Get the PDF text
    pub fn text(&self) -> String {
        let text_data = self.text_data.lock().unwrap();
        text_data.clone()
    }
    
    /// Get the total number of pages
    pub fn total_pages(&self) -> usize {
        self.total_pages
    }
    
    /// Jump to a specific page and optionally highlight a search term
    pub fn jump_to_page(&mut self, page_num: usize, search_term: Option<&str>, ctx: &Context) {
        if page_num < self.total_pages {
            self.current_page = page_num;
            
            // Render the page if poppler is available
            if self.use_poppler {
                self.render_page(self.current_page, ctx);
            }
            
            // Extract text for the page
            self.extract_page_text(self.current_page);
            
            // Enable text panel to show highlighting
            if search_term.is_some() {
                self.show_text_panel = true;
            }
        }
    }
    
    /// Show the PDF viewer
    pub fn show(&mut self, ui: &mut Ui, ctx: &Context) {
        // Process any loaded document
        self.process_loaded_document(ctx);
        
        // Load any rendered textures
        self.load_rendered_textures(ctx);
        
        // Handle keyboard navigation
        if self.document.is_some() {
            let input = ctx.input(|i| i.clone());
            let mut changed_page = false;
            
            if input.key_pressed(egui::Key::ArrowLeft) {
                // Previous page
                if self.current_page > 0 {
                    self.current_page = self.current_page.saturating_sub(1);
                    changed_page = true;
                }
            } else if input.key_pressed(egui::Key::ArrowRight) {
                // Next page
                if self.current_page < self.total_pages.saturating_sub(1) {
                    self.current_page = (self.current_page + 1).min(self.total_pages.saturating_sub(1));
                    changed_page = true;
                }
            } else if input.key_pressed(egui::Key::Home) {
                // First page
                if self.current_page > 0 {
                    self.current_page = 0;
                    changed_page = true;
                }
            } else if input.key_pressed(egui::Key::End) {
                // Last page
                if self.current_page < self.total_pages.saturating_sub(1) {
                    self.current_page = self.total_pages.saturating_sub(1);
                    changed_page = true;
                }
            }
            
            // If page changed, update the view
            if changed_page {
                // Render the page if poppler is available
                if self.use_poppler {
                    self.render_page(self.current_page, ctx);
                }
                // Extract text for the page
                self.extract_page_text(self.current_page);
                
                // Request repaint
                ctx.request_repaint();
            }
        }
        
        // Split the PDF viewer into top controls and content
        ui.vertical(|ui| {
            // Top panel with controls
            egui::TopBottomPanel::top("pdf_controls")
                .resizable(false)
                .frame(egui::Frame::none().fill(ui.style().visuals.panel_fill))
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Document title
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            if !self.document_title.is_empty() {
                                ui.label(RichText::new(&self.document_title).strong().heading());
                            }
                        });
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Open PDF button
                            if ui.button("📂 Open PDF...").clicked() {
                                if let Some(path) = Self::open_file_dialog() {
                                    self.load_pdf(&path);
                                }
                            }
                        });
                    });
                    
                    // Navigation controls
                    ui.horizontal(|ui| {
                        // Page navigation
                        if ui.add_enabled(self.current_page > 0, egui::Button::new("◀ Previous")).clicked() {
                            self.current_page = self.current_page.saturating_sub(1);
                            if self.use_poppler {
                                self.render_page(self.current_page, ctx);
                            }
                            self.extract_page_text(self.current_page);
                        }
                        
                        let total_pages = self.total_pages.max(1);
                        ui.label(format!("Page {} of {}", self.current_page + 1, total_pages));
                        
                        if ui.add_enabled(self.current_page < self.total_pages.saturating_sub(1), 
                                        egui::Button::new("Next ▶")).clicked() {
                            self.current_page = (self.current_page + 1).min(self.total_pages.saturating_sub(1));
                            if self.use_poppler {
                                self.render_page(self.current_page, ctx);
                            }
                            self.extract_page_text(self.current_page);
                        }
                        
                        // Keyboard shortcut hint
                        if self.total_pages > 1 {
                            ui.label(RichText::new("(←/→)").weak().small());
                        }
                        
                        // Zoom controls
                        ui.separator();
                        
                        if ui.add_enabled(self.zoom > 0.2, egui::Button::new("🔍-")).clicked() {
                            self.zoom = (self.zoom - 0.1).max(0.1);
                        }
                        
                        ui.label(format!("{:.0}%", self.zoom * 100.0));
                        
                        if ui.add_enabled(self.zoom < 3.0, egui::Button::new("🔍+")).clicked() {
                            self.zoom = (self.zoom + 0.1).min(3.0);
                        }
                        
                        ui.separator();
                        
                        // Option to toggle text panel
                        if ui.checkbox(&mut self.show_text_panel, "Show Text").clicked() {
                            // Toggle was clicked, no additional action needed
                        }
                        
                        // Show rendering status
                        if self.use_poppler {
                            ui.label(RichText::new("🖼️ Rendering").small().weak());
                        } else {
                            ui.label(RichText::new("📝 Text-only").small().weak());
                        }
                    });
                });
            
            // Main content area for the PDF
            // Show content if we have a document or if we're loading
            if self.document.is_some() || self.loading {
                // Render current page if not already rendered
                if self.use_poppler && !self.page_textures.contains_key(&self.current_page) {
                    let rendering = self.rendering_pages.lock().unwrap();
                    if !rendering.contains(&self.current_page) {
                        drop(rendering);
                        self.render_page(self.current_page, ctx);
                    }
                }
                
                // Display the PDF content
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    egui::ScrollArea::both()
                        .auto_shrink([false; 2])
                        .id_source("pdf_content")
                        .show(ui, |ui| {
                            // Try to show rendered page first
                            if let Some(texture) = self.page_textures.get(&self.current_page) {
                                // Show rendered page image
                                let size = texture.size_vec2() * self.zoom;
                                
                                ui.vertical_centered(|ui| {
                                    let image = egui::Image::new(texture)
                                        .fit_to_exact_size(size);
                                    ui.add(image);
                                });
                                
                                // Show text panel if requested
                                if self.show_text_panel {
                                    ui.separator();
                                    if let Some(page_data) = self.pages.get(&self.current_page) {
                                        egui::ScrollArea::vertical()
                                            .max_height(200.0)
                                            .show(ui, |ui| {
                                                ui.label(&page_data.text);
                                            });
                                    }
                                }
                            } else if let Some(page_data) = self.pages.get(&self.current_page) {
                                // Fallback to text display
                                let text_height = page_data.text.lines().count() as f32 * 18.0;
                                let content_rect = egui::Rect::from_min_size(
                                    ui.cursor().min,
                                    Vec2::new(
                                        page_data.size.x, 
                                        text_height.max(page_data.size.y)
                                    )
                                );
                                
                                // Create a "page" with white background
                                ui.painter().rect_filled(content_rect, 4.0, Color32::WHITE);
                                ui.painter().rect_stroke(content_rect, 4.0, egui::Stroke::new(1.0, Color32::GRAY));
                                ui.allocate_rect(content_rect, egui::Sense::hover());
                                
                                let text_rect = content_rect.shrink(20.0);
                                
                                if !page_data.text.is_empty() {
                                    // Show text with better formatting
                                    egui::ScrollArea::vertical()
                                        .max_height(content_rect.height())
                                        .show(ui, |ui| {
                                            ui.label(&page_data.text);
                                        });
                                } else {
                                    ui.put(text_rect, egui::Label::new("No text content available"));
                                }
                            } else {
                                // Loading state
                                let rendering = self.rendering_pages.lock().unwrap();
                                let is_rendering = rendering.contains(&self.current_page);
                                drop(rendering);
                                
                                ui.vertical_centered(|ui| {
                                    ui.add_space(50.0);
                                    if is_rendering {
                                        ui.label("Rendering page...");
                                    } else {
                                        ui.label("Loading page content...");
                                    }
                                    ui.add_space(50.0);
                                });
                            }
                        });
                });
            } else if self.loading {
                // Show loading indicator
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.label("Loading PDF...");
                    ui.add_space(100.0);
                });
            } else {
                // Show welcome screen when no document is loaded
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.heading("Welcome to PDFScan");
                    ui.add_space(20.0);
                    ui.label("To get started, open a PDF document.");
                    
                    
                    ui.add_space(20.0);
                    if ui.button("📂 Open PDF...").clicked() {
                        if let Some(path) = Self::open_file_dialog() {
                            self.load_pdf(&path);
                        }
                    }
                    ui.add_space(100.0);
                });
            }
        });
    }
    
    /// Show the document outline in the sidebar
    pub fn show_outline(&self, ui: &mut Ui) {
        if self.outline.is_empty() {
            ui.label("No outline available");
            return;
        }
        
        ui.heading("Document Outline");
        
        for item in &self.outline {
            self.show_outline_item(ui, item);
        }
    }
    
    /// Recursively show an outline item and its children
    fn show_outline_item(&self, ui: &mut Ui, item: &OutlineItem) {
        ui.horizontal(|ui| {
            // Indent based on level
            ui.add_space(item.level as f32 * 10.0);
            
            // Highlight if this is the current page
            let text = if item.page == self.current_page {
                RichText::new(&item.title).strong().color(ui.visuals().selection.stroke.color)
            } else {
                RichText::new(&item.title)
            };
            
            if ui.link(text).clicked() {
                // In a real implementation, this would scroll to the page
                // For now, we just set it as the current page
                // self.current_page = item.page;
            }
        });
        
        // Show children
        for child in &item.children {
            self.show_outline_item(ui, child);
        }
    }

    /// Open a file dialog
    fn open_file_dialog() -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("PDF Files", &["pdf"])
            .pick_file()
    }
}

/// Extract text from a PDF file using the pdf-extract library
fn extract_text_from_pdf(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let text = pdf_extract::extract_text_from_mem(&bytes)?;
    Ok(text)
} 