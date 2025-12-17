use std::path::Path;
use std::fs;
use egui::{Button, Color32, Ui};

/// Formats a file size for display
pub fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} bytes", size)
    }
}

/// Gets the file size
pub fn get_file_size(path: &Path) -> Option<u64> {
    fs::metadata(path).map(|m| m.len()).ok()
}

/// Truncates a string to a maximum length
pub fn truncate_string(s: &str, max_length: usize) -> String {
    if s.len() <= max_length {
        s.to_string()
    } else {
        format!("{}...", &s[..max_length - 3])
    }
}

/// Extracts the file extension from a path
pub fn get_file_extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
}

/// Checks if a file is a PDF
pub fn is_pdf(path: &Path) -> bool {
    get_file_extension(path) == "pdf"
}

/// Sets up styling for search buttons
pub fn setup_search_button_style<'a>(_ui: &mut Ui, button: Button<'a>) -> Button<'a> {
    button
        .fill(Color32::from_rgb(60, 120, 180))
        .stroke((1.0, Color32::from_rgb(40, 80, 120)))
} 