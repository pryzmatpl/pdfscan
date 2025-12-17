use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use rayon::prelude::*;
use walkdir::WalkDir;
use indicatif::ProgressBar;

/// Custom error type for statistical analysis operations
#[derive(Debug)]
pub enum StatsError {
    IoError(std::io::Error),
    PdfError(String),
    OtherError(String),
}

impl fmt::Display for StatsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatsError::IoError(e) => write!(f, "I/O error: {}", e),
            StatsError::PdfError(e) => write!(f, "PDF error: {}", e),
            StatsError::OtherError(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for StatsError {}

impl From<std::io::Error> for StatsError {
    fn from(err: std::io::Error) -> Self {
        StatsError::IoError(err)
    }
}

/// Represents a document with its keyword occurrences and correlation
#[derive(Debug)]
struct Document {
    filename: String,
    keyword_counts: HashMap<String, usize>,
    correlation_score: f64,
}

/// Represents a keyword analysis result
#[derive(Debug)]
pub struct KeywordAnalysis {
    keywords: Vec<String>,
    documents: Vec<Document>,
    correlations: Vec<Vec<f64>>,
    total_documents: usize,
}

impl KeywordAnalysis {
    fn new(keywords: Vec<String>) -> Self {
        let keyword_count = keywords.len();
        Self {
            keywords,
            documents: Vec::new(),
            correlations: vec![vec![0.0; keyword_count]; keyword_count],
            total_documents: 0,
        }
    }

    /// Calculate keyword correlations across all documents
    fn calculate_correlations(&mut self) {
        // Reset correlations
        let keyword_count = self.keywords.len();
        self.correlations = vec![vec![0.0; keyword_count]; keyword_count];
        
        // Calculate co-occurrences
        for doc in &self.documents {
            for (i, k1) in self.keywords.iter().enumerate() {
                let count1 = *doc.keyword_counts.get(k1).unwrap_or(&0);
                if count1 > 0 {
                    for (j, k2) in self.keywords.iter().enumerate().skip(i + 1) {
                        let count2 = *doc.keyword_counts.get(k2).unwrap_or(&0);
                        if count2 > 0 {
                            // Use minimum of the two counts as correlation strength
                            let min_count = std::cmp::min(count1, count2) as f64;
                            self.correlations[i][j] += min_count;
                        }
                    }
                }
            }
        }
        
        // Normalize correlations by document count
        if !self.documents.is_empty() {
            let doc_count = self.documents.len() as f64;
            for i in 0..keyword_count {
                for j in (i + 1)..keyword_count {
                    self.correlations[i][j] /= doc_count;
                }
            }
        }
    }

    /// Rank documents based on keyword correlations
    fn rank_documents(&mut self, threshold: f64) -> Vec<(String, f64)> {
        // Calculate weights for documents based on correlations
        for doc in &mut self.documents {
            let mut score = 0.0;
            
            for (i, k1) in self.keywords.iter().enumerate() {
                let count1 = *doc.keyword_counts.get(k1).unwrap_or(&0);
                if count1 > 0 {
                    for (j, k2) in self.keywords.iter().enumerate().skip(i + 1) {
                        let count2 = *doc.keyword_counts.get(k2).unwrap_or(&0);
                        if count2 > 0 && self.correlations[i][j] >= threshold {
                            // Add correlation score to document weight
                            let correlation_strength = self.correlations[i][j];
                            score += correlation_strength * (count1.min(count2) as f64);
                        }
                    }
                }
            }
            
            doc.correlation_score = score;
        }
        
        // Sort documents by score
        let mut ranked_docs = self.documents.iter()
            .map(|doc| (doc.filename.clone(), doc.correlation_score))
            .collect::<Vec<_>>();
        
        ranked_docs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked_docs
    }

    /// Generate correlation matrix visualization (text-based)
    fn generate_correlation_matrix(&self) -> String {
        let mut result = String::new();
        
        // Header with keyword indices
        result.push_str("Keyword Correlation Matrix:\n\n");
        result.push_str("    ");
        for i in 0..self.keywords.len() {
            result.push_str(&format!("{:<4}", i));
        }
        result.push('\n');
        
        // Correlation values
        for i in 0..self.keywords.len() {
            result.push_str(&format!("{:<3} ", i));
            for j in 0..self.keywords.len() {
                if i < j {
                    result.push_str(&format!("{:.2} ", self.correlations[i][j]));
                } else if i > j {
                    result.push_str(&format!("{:.2} ", self.correlations[j][i]));
                } else {
                    result.push_str(&format!("---- "));
                }
            }
            result.push('\n');
        }
        
        // Keyword index mapping
        result.push_str("\nKeyword Index Mapping:\n");
        for (i, keyword) in self.keywords.iter().enumerate() {
            result.push_str(&format!("{}: {}\n", i, keyword));
        }
        
        result
    }
}

/// Run statistical analysis on PDF files
pub fn run(
    input_paths: &[String],
    keywords: &[String],
    output_file: &str,
    correlation_threshold: f64,
) -> Result<(), Box<dyn Error>> {
    if keywords.is_empty() {
        return Err(Box::new(StatsError::OtherError(
            "No keywords provided for analysis".to_string()
        )));
    }
    
    // Collect PDF paths
    let pdf_paths = collect_pdf_paths(input_paths)?;
    
    if pdf_paths.is_empty() {
        return Err(Box::new(StatsError::OtherError(
            "No PDF files found in the provided paths".to_string()
        )));
    }
    
    // Create progress bar
    let pb = ProgressBar::new(pdf_paths.len() as u64);
    pb.set_message("Analyzing PDFs");
    
    // Initialize keyword analysis
    let mut analysis = KeywordAnalysis::new(keywords.iter().map(|s| s.to_string()).collect());
    
    // Process PDFs in parallel
    let documents: Vec<Document> = pdf_paths.par_iter()
        .map(|path| {
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            
            let keyword_counts = match extract_keyword_counts(path, keywords) {
                Ok(counts) => counts,
                Err(e) => {
                    eprintln!("Error processing {}: {}", filename, e);
                    HashMap::new()
                }
            };
            
            pb.inc(1);
            
            Document {
                filename,
                keyword_counts,
                correlation_score: 0.0,
            }
        })
        .collect();
    
    pb.finish_with_message("Analysis complete");
    
    // Update analysis with documents
    analysis.documents = documents;
    analysis.total_documents = analysis.documents.len();
    
    // Calculate correlations
    analysis.calculate_correlations();
    
    // Rank documents
    let ranked_docs = analysis.rank_documents(correlation_threshold);
    
    // Generate report
    let correlation_matrix = analysis.generate_correlation_matrix();
    
    // Create report
    let mut report = String::new();
    report.push_str(&format!("PDFScan Statistical Analysis Report\n"));
    report.push_str(&format!("================================\n\n"));
    report.push_str(&format!("Keywords: {}\n", keywords.join(", ")));
    report.push_str(&format!("Total documents analyzed: {}\n", analysis.total_documents));
    report.push_str(&format!("Correlation threshold: {:.2}\n\n", correlation_threshold));
    
    report.push_str(&correlation_matrix);
    report.push_str("\n\nRanked Documents by Keyword Correlation:\n");
    report.push_str("==========================================\n");
    
    for (i, (filename, score)) in ranked_docs.iter().enumerate().take(20) {
        if *score > 0.0 {
            report.push_str(&format!("{}. {} (score: {:.2})\n", i+1, filename, score));
        }
    }
    
    // Write to output file
    fs::write(output_file, report)?;
    
    println!("Successfully generated statistical analysis report in '{}'", output_file);
    Ok(())
}

/// Collect all PDF file paths from the provided input paths
fn collect_pdf_paths(input_paths: &[String]) -> Result<Vec<PathBuf>, StatsError> {
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

/// Extract keyword counts from a PDF file
fn extract_keyword_counts(path: &Path, keywords: &[String]) -> Result<HashMap<String, usize>, StatsError> {
    let bytes = fs::read(path)?;
    
    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| StatsError::PdfError(
            format!("Error extracting text from {}: {}", path.display(), e)
        ))?;
    
    let mut counts = HashMap::new();
    
    for keyword in keywords {
        let count = text.matches(keyword).count();
        counts.insert(keyword.to_string(), count);
    }
    
    Ok(counts)
} 