cle# Learning Rust by Building a Video Editor from Scratch

Welcome to your journey of building a video editor in Rust from scratch. This document is your blueprint and mentor combined — structured to simulate the learning and collaborative environment you'd experience working as a junior engineer under the guidance of a senior developer.

**This is an **``** document intended to be followed phase by phase. Your coding agent should ALWAYS return to this file after each completed phase to confirm progress and align on next steps.**

---

## 🧭 Overview

We'll be building a minimal video editing tool using Rust, progressing through:

1. Rust Fundamentals and Environment Setup
2. CLI-based Skeleton App
3. Basic Video Decoding and Metadata Access
4. Frame Extraction and Editing
5. Exporting and (optionally) GUI visualization

Each phase has:

- 🎯 Goals
- 📚 Learning Topics
- 🛠️ Tasks
- 🔧 Code Prompts (Senior Dev pre-written skeletons with **TODOs** for you to fill in)

---

## Phase 0: Rust Setup & Fundamentals

### 🎯 Goals

- Install Rust tooling
- Learn basic Rust syntax and mental model
- Build your first utility app

### 📚 Topics Covered

- Ownership, borrowing, lifetimes
- Modules & error handling
- Project structure using Cargo

### 🛠️ Tasks

1. Install Rust: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
2. Create a new project:
   ```bash
   cargo new rust_video_editor
   cd rust_video_editor
   cargo run
   ```
3. Learn the difference between `String` and `&str` and write your own explanation.
4. Build a CLI app to list all filenames in a directory.

### 🔧 Code Prompt

```rust
use std::fs;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let dir = &args[1];

    // TODO: List files in the directory and print them using fs::read_dir
}
```

---

## Phase 1: Building a CLI Skeleton

### 🎯 Goals

- Design command structure for your video editor
- Parse arguments using `clap`

### 📚 Topics Covered

- Enums, match statements
- CLI structuring with `clap`

### 🛠️ Tasks

1. Add `clap` to `Cargo.toml`:

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
```

2. Define CLI commands:
   - `load <filename>`
   - `cut <start> <end>`
   - `export <outputfile>`

### 🔧 Code Prompt

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "Rust Video Editor")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Load { filename: String },
    Cut { start: u32, end: u32 },
    Export { output: String },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Load { filename } => {
            // TODO: load video logic here
        },
        Commands::Cut { start, end } => {
            // TODO: cut video logic here
        },
        Commands::Export { output } => {
            // TODO: export video logic here
        },
    }
}
```

---

## Phase 2: Video Processing Integration

### 🎯 Goals

- Integrate a video decoding crate
- Access metadata from video files

### 📚 Topics Covered

- Working with crates (ffmpeg-next or ffmpeg-sys)
- Propagating errors with `Result<T, E>`

### 🛠️ Tasks

1. Install and set up `ffmpeg-next`
2. Initialize FFmpeg and extract video stream info

### 🔧 Code Prompt

```rust
use ffmpeg_next as ffmpeg;

fn load_video(filename: &str) -> Result<(), ffmpeg::Error> {
    ffmpeg::init()?;
    // TODO: Open video and print metadata
    Ok(())
}
```

---

## Phase 3: Implementing Editing Logic

### 🎯 Goals

- Implement a way to cut parts of a video
- Allow exporting a new video file

### 📚 Topics Covered

- Safe memory handling
- Stream decoding/encoding logic
- File I/O for video frames

### 🛠️ Tasks

1. Use timestamps to select segments of video
2. Re-encode and write output using FFmpeg

### 🔧 Code Prompt

```rust
fn cut_video(input: &str, start: u32, end: u32, output: &str) -> Result<(), String> {
    // TODO: extract frames between timestamps and write to output
    Ok(())
}
```

---

## Phase 4: (Optional) GUI/TUI Frontend

### 🎯 Goals

- Add basic UI to visualize timeline and edits

### 📚 Topics Covered

- GUI design using `egui`, `dioxus`, or `ratatui`
- Event-driven architecture in Rust

### 🛠️ Optional Tools

- `egui` for a graphical editor
- `ratatui` for terminal-based editor

---

## 👨‍🏫 Agent Instructions

- After **each phase**, come back to this `.md` file
- Summarize what was completed and confirm readiness for the next phase
- Leave some implementation areas for the user as **TODO**
- Ask conceptual questions to ensure the user understands
- Reinforce Rust best practices (e.g., immutability by default, `Result`, lifetimes)

---