use clap::{Subcommand, Parser};
use ffmpeg_next as ffmpeg;

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

            ffmpeg::init().unwrap();

            if let Ok(context) = ffmpeg::format::input(&filename) {
                println!("Duration: {:?}", context.duration());
                println!("Streams:");

                for (idx, stream) in context.streams().enumerate() {
                    let params = stream.parameters();
                    println!(
                        "  Stream {}: codec_type={:?}, codec_id={:?}",
                        idx,
                        params.medium(),
                        params.id()
                    );
                }
            } else {
                println!("Failed to load video file: {}", filename);
            }
            // Here you would add the logic to load the video file
        }
        Commands::Export { output } => {
            println!("Exporting video to: {}", output);
            // Here you would add the logic to export the video
        }
    }
}

