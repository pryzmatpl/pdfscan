use eframe::{NativeOptions, run_native};
use egui::ViewportBuilder;

mod gui;
mod extract;
mod search;
mod stats;

fn main() -> Result<(), eframe::Error> {
    // Initialize logging
    env_logger::init();
    
    // Set up native options
    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("PDFScan"),
        ..Default::default()
    };
    
    // Run the app
    run_native(
        "PDFScan",
        options,
        Box::new(|cc| Box::new(crate::gui::PdfScanApp::new(cc)))
    )
} 