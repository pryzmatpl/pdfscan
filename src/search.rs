use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::error::Error;
use std::fmt;
use walkdir::WalkDir;
use zip::write::FileOptions;
use chrono;
use dirs;

/// Custom error type for search operations
#[derive(Debug)]
pub enum SearchError {
    IoError(io::Error),
    PdfError(String),
    ZipError(zip::result::ZipError),
    OtherError(String),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::IoError(e) => write!(f, "I/O error: {}", e),
            SearchError::PdfError(e) => write!(f, "PDF error: {}", e),
            SearchError::ZipError(e) => write!(f, "Zip error: {}", e),
            SearchError::OtherError(e) => write!(f, "{}", e),
        }
    }
}

impl Error for SearchError {}

impl From<io::Error> for SearchError {
    fn from(err: io::Error) -> Self {
        SearchError::IoError(err)
    }
}

impl From<zip::result::ZipError> for SearchError {
    fn from(err: zip::result::ZipError) -> Self {
        SearchError::ZipError(err)
    }
}

/// Main function to run the search functionality
pub fn run(search_phrase: &str, directories: &[PathBuf], zip_output: bool) -> Result<(), Box<dyn Error>> {
    let search_dirs = if directories.is_empty() {
        // Use home directory as default if no directories provided
        match dirs::home_dir() {
            Some(home_dir) => vec![home_dir],
            None => return Err(Box::new(SearchError::OtherError(
                "Unable to determine the user's home directory".to_string()
            ))),
        }
    } else {
        directories.to_vec()
    };

    // Validate all directories exist
    for dir in &search_dirs {
        if !dir.is_dir() {
            return Err(Box::new(SearchError::OtherError(
                format!("Path is not a directory: {}", dir.display())
            )));
        }
    }

    // Search for PDF files
    let results = search_pdf_files(search_phrase, &search_dirs)?;
    
    // Output results
    println!("\nFound {} matching PDF files:", results.len());
    for result in &results {
        println!("{}", result);
    }

    // Create zip file if requested
    if zip_output && !results.is_empty() {
        let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
        let zip_file_name = format!("search_results_{}.zip", timestamp);
        
        zip_files(&zip_file_name, &results)?;
        println!("Search results have been zipped to: {}", zip_file_name);
    }

    Ok(())
}

/// Search for PDF files containing the given phrase
fn search_pdf_files(search_phrase: &str, directories: &[PathBuf]) -> Result<Vec<String>, SearchError> {
    // Using Arc<Mutex<Vec<String>>> to safely share results between threads
    let results: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for directory in directories {
        let results_clone = results.clone();
        let search_phrase_clone = search_phrase.to_string();
        let directory_clone = directory.clone();

        handles.push(thread::spawn(move || {
            println!("Searching in: {}", directory_clone.display());
            search_directory(&directory_clone, &search_phrase_clone, results_clone);
        }));
    }

    // Wait for all threads to complete
    for handle in handles {
        if let Err(e) = handle.join() {
            eprintln!("A search thread panicked: {:?}", e);
        }
    }

    // Return the final results
    let locked_results = results.lock()
        .map_err(|_| SearchError::OtherError("Failed to lock results".to_string()))?;
    
    Ok(locked_results.clone())
}

/// Search for PDFs in a single directory
fn search_directory(dir: &PathBuf, search_phrase: &str, results: Arc<Mutex<Vec<String>>>) {
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == "pdf" {
                    let path_str = path.to_string_lossy().into_owned();
                    
                    // If no search phrase specified, include all PDFs
                    if search_phrase.is_empty() {
                        let mut locked_results = results.lock().unwrap();
                        locked_results.push(path_str);
                        continue;
                    }

                    // Check if PDF contains the search phrase
                    match search_phrase_in_pdf(path, search_phrase) {
                        Ok(true) => {
                            let mut locked_results = results.lock().unwrap();
                            locked_results.push(path_str);
                        },
                        Ok(false) => {}, // Phrase not found
                        Err(e) => eprintln!("Error processing {}: {}", path.display(), e),
                    }
                }
            }
        }
    }
}

/// Check if a PDF file contains the search phrase
fn search_phrase_in_pdf(file_path: &Path, search_phrase: &str) -> Result<bool, SearchError> {
    let bytes = std::fs::read(file_path)?;
    
    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| SearchError::PdfError(
            format!("Error extracting text from {}: {}", file_path.display(), e)
        ))?;

    Ok(text.contains(search_phrase))
}

/// Create a zip file containing the specified PDF files
pub fn zip_files(zip_file_name: &str, file_paths: &[String]) -> Result<(), SearchError> {
    let path = Path::new(zip_file_name);
    let file = File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);

    for file_path in file_paths {
        let path = Path::new(file_path);
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| SearchError::OtherError(
                format!("Invalid filename in path: {}", file_path)
            ))?;

        let mut file_content = Vec::new();
        let mut file = File::open(path)?;
        file.read_to_end(&mut file_content)?;

        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        zip.start_file(file_name, options)?;
        zip.write_all(&file_content)?;
    }

    zip.finish()?;
    Ok(())
} 