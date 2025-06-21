use std::fs;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} <directory>", args[0]);
        return;
    }

    let dir = &args[1];

    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    println!("{}", entry.file_name().to_string_lossy());
                }
            }
        }
        Err(e) => {
            println!("Error reading directory: {}", e);
        }
    }
}
