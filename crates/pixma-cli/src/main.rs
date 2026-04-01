mod discover;
mod print;
mod scan;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "pixma", version, about = "Canon PIXMA driver")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Find Canon printers on the network
    Discover {
        #[arg(short, long, default_value_t = 5)]
        timeout: u64,
    },

    /// Scan a document from the flatbed
    Scan {
        /// Output file path
        output: PathBuf,

        /// Resolution in DPI
        #[arg(short, long, default_value_t = 300)]
        resolution: u16,

        /// Color mode: color, grayscale
        #[arg(short, long, default_value = "color")]
        color: String,

        /// Output format: png, jpeg (inferred from extension if omitted)
        #[arg(short, long)]
        format: Option<String>,

        /// Device IP address (auto-discovers if omitted)
        #[arg(short, long)]
        device: Option<String>,
    },

    /// Print a file via IPP
    Print {
        /// File to print (PDF, PNG, JPEG)
        file: String,

        /// Device IP address (auto-discovers if omitted)
        #[arg(short, long)]
        device: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Discover { timeout } => discover::run(timeout).await,
        Commands::Scan {
            output,
            resolution,
            color,
            format,
            device,
        } => scan::run(output, resolution, color, format, device).await,
        Commands::Print { file, device } => print::run(file, device).await,
    }
}
