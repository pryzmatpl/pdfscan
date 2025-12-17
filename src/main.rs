use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

mod extract;
mod search;
mod stats;

#[derive(Parser)]
#[command(author, version, about = "PDF text extraction and search tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract text from PDFs and save to a file
    Extract {
        /// Output text file path
        output_file: String,
        
        /// Input paths (directories or PDF files)
        input_paths: Vec<String>,
    },
    
    /// Search for text in PDF files
    Search {
        /// Text to search for
        #[arg(short, long)]
        search_phrase: String,
        
        /// Directories to search in
        #[arg(short, long, required = false)]
        directories: Vec<PathBuf>,
        
        /// Enable ZIP output of matching files
        #[arg(short, long)]
        zip: bool,
    },

    /// Analyze keyword correlations in PDF files
    Analyze {
        /// Keywords to analyze
        #[arg(short, long, required = true)]
        keywords: Vec<String>,
        
        /// Input paths (directories or PDF files)
        #[arg(short, long, required = true)]
        input_paths: Vec<String>,
        
        /// Output report file path
        #[arg(short, long, default_value = "pdf_analysis_report.txt")]
        output_file: String,
        
        /// Correlation threshold (0.0 to 1.0)
        #[arg(short, long, default_value_t = 0.1)]
        threshold: f64,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Extract { output_file, input_paths } => {
            extract::run(&output_file, &input_paths)
        },
        Commands::Search { search_phrase, directories, zip } => {
            search::run(&search_phrase, &directories, zip)
        },
        Commands::Analyze { keywords, input_paths, output_file, threshold } => {
            stats::run(&input_paths, &keywords, &output_file, threshold)
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
} 