use indicatif::ProgressBar;
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::io;
use walkdir::WalkDir;
use std::error::Error;
use std::fmt;

/// Custom error type for extraction operations
#[derive(Debug)]
pub enum ExtractError {
    IoError(io::Error),
    PdfError(String),
    OtherError(String),
}

impl fmt::Display for ExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtractError::IoError(e) => write!(f, "I/O error: {}", e),
            ExtractError::PdfError(e) => write!(f, "PDF extraction error: {}", e),
            ExtractError::OtherError(e) => write!(f, "{}", e),
        }
    }
}

impl Error for ExtractError {}

impl From<io::Error> for ExtractError {
    fn from(err: io::Error) -> Self {
        ExtractError::IoError(err)
    }
}

/// Main function to run the extraction functionality
pub fn run(output_file: &str, input_paths: &[String]) -> Result<(), Box<dyn Error>> {
    // Collect all PDF paths
    let pdf_paths = collect_pdf_paths(input_paths)?;
    
    if pdf_paths.is_empty() {
        return Err(Box::new(ExtractError::OtherError(
            "No PDF files found in the provided paths".to_string()
        )));
    }

    // Create progress bar
    let pb = ProgressBar::new(pdf_paths.len() as u64);
    pb.set_message("Processing PDFs");

    // Process PDFs in parallel
    let extracted_texts = process_pdfs(&pdf_paths, &pb);
    
    // Finish progress bar
    pb.finish_with_message("Done");

    // Write to output file
    fs::write(output_file, extracted_texts.join("\n"))?;
    
    println!("Successfully extracted text from {} PDFs to '{}'", pdf_paths.len(), output_file);
    Ok(())
}

/// Collect all PDF file paths from the provided input paths
fn collect_pdf_paths(input_paths: &[String]) -> Result<Vec<PathBuf>, ExtractError> {
    let mut pdf_paths: Vec<PathBuf> = Vec::new();
    
    for path in input_paths {
        let path = PathBuf::from(path);
        if path.is_dir() {
            for entry in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("pdf") {
                    pdf_paths.push(entry.path().to_path_buf());
                }
            }
        } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("pdf") {
            pdf_paths.push(path);
        } else {
            eprintln!("Warning: Skipping invalid path: {}", path.display());
        }
    }

    // Remove duplicates
    pdf_paths.sort();
    pdf_paths.dedup();
    
    Ok(pdf_paths)
}

/// Process PDFs in parallel and extract text
fn process_pdfs(pdf_paths: &[PathBuf], pb: &ProgressBar) -> Vec<String> {
    pdf_paths
        .par_iter()
        .map(|path| {
            let filename = path.file_name().unwrap().to_str().unwrap();
            match extract_text_from_pdf(path) {
                Ok(text) => format!(
                    "[Start of document: {}]\n{}\n[End of document: {}]\n",
                    filename, text, filename
                ),
                Err(e) => {
                    eprintln!("Error processing {}: {}", filename, e);
                    String::new()
                }
            }
        })
        .inspect(|_| pb.inc(1))
        .collect()
}

/// Extract text from a single PDF file
fn extract_text_from_pdf(path: &PathBuf) -> Result<String, ExtractError> {
    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
        
    let bytes = fs::read(path)
        .map_err(|e| ExtractError::IoError(e))?;
        
    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| ExtractError::PdfError(format!("Error extracting text from {}: {}", filename, e)))?;
        
    Ok(text)
} 