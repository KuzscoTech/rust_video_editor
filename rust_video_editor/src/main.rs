use clap::{Subcommand, Parser};

#[derive(Parser)]
#[command(name = "Rust Video Editor")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Load { filename: String },
    Export {output: String },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Load { filename } => {
            println!("Loading video file: {}", filename);
            // Here you would add the logic to load the video file
        }
        Commands::Export { output } => {
            println!("Exporting video to: {}", output);
            // Here you would add the logic to export the video
        }
    }
}

