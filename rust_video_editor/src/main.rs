use clap::{Parser, Subcommand};
use ffmpeg_next as ffmpeg;
use std::collections::HashMap;
use ffmpeg::{format};
use std::fs;

#[derive(Parser)]
#[command(name = "Rust Video Editor")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Load {
        filename: String,
    },
    Export {
        output: String,
    },
    Cut {
        input: String,
        start: u32,
        end: u32,
        output: String,
    },
    RemoveSilence {
        input: String,
        threshold: f64,
        output: String,
    },
}

fn cut_noisy_segments(input: &str, threshold:f64, output: &str) -> Result<(), String> {
    // Vector?
    let mut keep_intervals = find_noisy_intervals(input, threshold)?;
    
    if keep_intervals.is_empty() {
        println!("No non noisy segments found.");
        return Ok(());
    } else {
            // Remove zero-length and duplicate intervals
        keep_intervals.retain(|(s, e)| e > s);
        keep_intervals.dedup();
        // Filter intervals based on minimum duration
        let min_duration = 1; // seconds
        keep_intervals.retain(|(s, e)| e - s >= min_duration);

        // Merge overlapping or adjacent intervals
        keep_intervals.sort_by_key(|&(s, _)| s);
        let mut merged_intervals: Vec<(u32, u32)> = Vec::new();
        for (s, e) in keep_intervals {
            if let Some((_, last_e)) = merged_intervals.last_mut() {
                if s <= *last_e + 1 {
                    // Merge intervals if overlapping or adjacent
                    *last_e = (*last_e).max(e);
                    continue;
                }
            }
            merged_intervals.push((s, e));
        }
        // Print merged intervals for debugging
        // println!("Merged intervals: {:?}", merged_intervals);
        // Filter intervals based on minimum duration (e.g., 2 seconds)
        let min_duration = 2;
        // println!("Filtered intervals (>{}s): {:?}", min_duration, merged_intervals);
        let mut segment_paths = Vec::new();
        let mut index = 0;
        let mut total_segment_duration = 0.0;
        for (start, end) in merged_intervals {
            let base = output.trim_end_matches(".mp4");
            // Find the next keyframe after start
            let keyframe_start = find_next_keyframe(input, start).unwrap_or(start);
            // Only cut if the keyframe is strictly less than end
            if keyframe_start >= end {
                continue; // skip if no keyframe in interval
            }
            // Align cut to the exact keyframe timestamp
            let segment_output = format!("{}_{}.mp4", base, index);
            match cut_video(input, keyframe_start, end, &segment_output) {
                Ok(()) => {
                    // Filter out zero-length or <=2-frame video segments
                    let mut video_packet_count = 0;
                    let mut seg_duration = 0.0;
                    if let Ok(mut seg_file) = format::input(&segment_output) {
                        seg_duration = seg_file.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64;
                        for (stream, _) in seg_file.packets() {
                            if stream.parameters().medium() == ffmpeg::media::Type::Video {
                                video_packet_count += 1;
                                if video_packet_count > 2 {
                                    break;
                                }
                            }
                        }
                    }
                    // println!("Segment {}: {}-{}s, duration: {:.2}s, video frames: {}", index, start, end, seg_duration, video_packet_count);
                    total_segment_duration += seg_duration;
                    if video_packet_count <= 2 {
                        // println!("Deleting segment {}: zero-length or <=2-frame video", segment_output);
                        let _ = std::fs::remove_file(&segment_output);
                        continue;
                    }
                    segment_paths.push(segment_output.clone());
                    index += 1;
                    // println!("Segments created: {}", index);
                }
                Err(_e) => {
                    // println!("Skipping segment {}: {}", index, e);
                    let _ = std::fs::remove_file(&segment_output);
                }
            }
        }
        // println!("Total segment duration before join: {:.2}s", total_segment_duration);

        if index == 0 {
            // println!("No segments to join. Output file will not be created.");
            return Err("No noisy segments found to join.".to_string());
        }

        //Recompile the output into a single video file
        let mut output_file = format::output(output).map_err(|e| e.to_string())?;
        let mut out_stream_indices: HashMap<usize, usize> = HashMap::new();

        if index > 0 {
            let first_segment_input = &segment_paths[0];
            let first_segment_file = format::input(first_segment_input).map_err(|e| e.to_string())?;

            for (stream_index, stream) in first_segment_file.streams().enumerate() {
                let codec_id = stream.parameters().id();
                if codec_id == ffmpeg::codec::Id::None {
                    println!("Skipping stream {}: codec is None", stream_index);
                    continue;
                }
                let mut out_stream = output_file.add_stream(codec_id).map_err(|e| e.to_string())?;
                out_stream.set_parameters(stream.parameters());
                out_stream_indices.insert(stream_index, out_stream.index());
            }
        }

        output_file.write_header().map_err(|e| e.to_string())?;

        let mut last_pts: HashMap<usize, i64> = HashMap::new();
        let mut last_dts: HashMap<usize, i64> = HashMap::new();
        let mut first_packet_written: HashMap<usize, bool> = HashMap::new();

        for segment_input in &segment_paths {
            let mut segment_file = format::input(segment_input).map_err(|e| e.to_string())?;
            // Collect all packets, sort by pts (for proper interleaving)
            let mut packets: Vec<(usize, ffmpeg::Packet)> = Vec::new();
            for (packet_stream, packet) in segment_file.packets() {
                packets.push((packet_stream.index(), packet));
            }
            packets.sort_by_key(|(_, packet)| packet.pts().unwrap_or(0));

            // Track first pts/dts per stream in this segment
            let mut segment_first_pts: HashMap<usize, i64> = HashMap::new();
            let mut segment_first_dts: HashMap<usize, i64> = HashMap::new();
            for (stream_index, packet) in &packets {
                if !segment_first_pts.contains_key(stream_index) {
                    if let Some(pts) = packet.pts() {
                        segment_first_pts.insert(*stream_index, pts);
                    }
                }
                if !segment_first_dts.contains_key(stream_index) {
                    if let Some(dts) = packet.dts() {
                        segment_first_dts.insert(*stream_index, dts);
                    }
                }
            }

            // Calculate offset ONCE per segment per stream
            let mut segment_pts_offset: HashMap<usize, i64> = HashMap::new();
            let mut segment_dts_offset: HashMap<usize, i64> = HashMap::new();
            for stream_index in segment_first_pts.keys() {
                let out_stream_index = *out_stream_indices.get(stream_index).unwrap();
                let last_pts_val = *last_pts.get(&out_stream_index).unwrap_or(&-1);
                let last_dts_val = *last_dts.get(&out_stream_index).unwrap_or(&-1);
                let seg_first_pts = *segment_first_pts.get(stream_index).unwrap_or(&0);
                let seg_first_dts = *segment_first_dts.get(stream_index).unwrap_or(&0);

                let pts_offset = last_pts_val + 1 - seg_first_pts;
                let dts_offset = last_dts_val + 1 - seg_first_dts;
                segment_pts_offset.insert(*stream_index, pts_offset);
                segment_dts_offset.insert(*stream_index, dts_offset);
                // println!("[JOIN] Stream {}: segment_pts_offset = {} dts_offset = {} (last_pts {} seg_first_pts {} last_dts {} seg_first_dts {})", out_stream_index, pts_offset, dts_offset, last_pts_val, seg_first_pts, last_dts_val, seg_first_dts);
            }

            let mut seen_first_keyframe: HashMap<usize, bool> = HashMap::new();
            for (stream_index, mut packet) in packets {
                if let Some(&out_stream_index) = out_stream_indices.get(&stream_index) {
                    let stream = segment_file.stream(stream_index).unwrap();
                    // For video, skip until first keyframe
                    if stream.parameters().medium() == ffmpeg::media::Type::Video {
                        let seen = seen_first_keyframe.entry(stream_index).or_insert(false);
                        if !*seen {
                            if !packet.is_key() {
                                continue;
                            }
                            *seen = true;
                        }
                    }
                    // Apply offset ONCE per packet
                    if let Some(offset) = segment_pts_offset.get(&stream_index) {
                        if let Some(pts) = packet.pts() {
                            let candidate = pts + offset;
                            // println!("[JOIN] Stream {}: old PTS {} -> new PTS {} (offset {})", out_stream_index, pts, candidate, offset);
                            packet.set_pts(Some(candidate));
                            last_pts.insert(out_stream_index, candidate);
                        }
                    }
                    if let Some(offset) = segment_dts_offset.get(&stream_index) {
                        if let Some(dts) = packet.dts() {
                            let candidate = dts + offset;
                            // println!("[JOIN] Stream {}: old DTS {} -> new DTS {} (offset {})", out_stream_index, dts, candidate, offset);
                            packet.set_dts(Some(candidate));
                            last_dts.insert(out_stream_index, candidate);
                        }
                    }
                    packet.set_stream(out_stream_index);
                    packet.write_interleaved(&mut output_file).map_err(|e| e.to_string())?;
                } else {
                    println!("Skipping stream {} in segment {}: not present in output file", stream_index, segment_input);
                }
            }
        }
        output_file.write_trailer().map_err(|e| e.to_string())?;

        // Remove all segment files after joining
        for seg_path in segment_paths {
            let _ = std::fs::remove_file(seg_path);
        }
        Ok(())
    }
}

fn find_noisy_intervals(input: &str, threshold: f64) -> Result<Vec<(u32, u32)>, String> {
    ffmpeg::init().map_err(|e| e.to_string())?;
    let mut input_file = format::input(&input).map_err(|e| e.to_string())?;
    let mut noisy_intervals = Vec::new();

    let streams: Vec<_> = input_file.streams().enumerate()
        .filter(|(_, stream)| stream.parameters().medium() == ffmpeg::media::Type::Audio)
        .map(|(stream_index, stream)| (stream_index, stream.parameters().clone()))
        .collect();

    let mut current_start: Option<u32> = None;
    let mut last_end: Option<u32> = None;

    for (stream_index, codec_params) in streams {
        let mut decoder = ffmpeg::codec::context::Context::from_parameters(codec_params)
            .and_then(|ctx| ctx.decoder().audio())
            .map_err(|e| e.to_string())?;

        for (packet_stream, packet) in input_file.packets() {
            if packet_stream.index() == stream_index {
                decoder.send_packet(&packet).map_err(|e| e.to_string())?;
                let mut frame = ffmpeg::frame::Audio::empty();
                while decoder.receive_frame(&mut frame).is_ok() {
                    let time_base = packet_stream.time_base();
                    let start_pts = packet.pts().unwrap_or(0);
                    let start_sec = start_pts as f64 * time_base.numerator() as f64 / time_base.denominator() as f64;
                    let frame_duration_sec = frame.samples() as f64 / decoder.rate() as f64;
                    let end_sec = start_sec + frame_duration_sec;
                    let start_u32 = start_sec as u32;
                    let end_u32 = end_sec as u32;

                    if is_noisy(&frame, threshold) {
                        if current_start.is_none() {
                            current_start = Some(start_u32);
                        }
                        last_end = Some(end_u32);
                    } else {
                        if let (Some(s), Some(e)) = (current_start, last_end) {
                            noisy_intervals.push((s, e));
                            current_start = None;
                            last_end = None;
                        }
                    }
                }
            }
        }
    }
    // Push the last interval if the file ends while still in a noisy segment
    if let (Some(s), Some(e)) = (current_start, last_end) {
        noisy_intervals.push((s, e));
    }
    println!("Noisy intervals: {:?}", noisy_intervals);
    for (s, e) in &noisy_intervals {
        // println!("Interval: {} - {} ({}s)", s, e, e - s);
    }
    Ok(noisy_intervals)
}

fn is_noisy(audio_frame: &ffmpeg::frame::Audio, threshold: f64) -> bool {
    match audio_frame.format().name() {
        "s16" => {
            let data = audio_frame.data(0);
            let samples: Vec<i16> = data
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect();
            let sum_squares: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
            let rms = (sum_squares / samples.len() as f64).sqrt();
            let db = 20.0 * (rms.abs().log10());
            // println!("Frame dB: {}", db);
            db > threshold
        }
        "fltp" => {
            let nb_channels = audio_frame.channels();
            let mut sum_squares = 0.0;
            let mut count = 0;
            for ch in 0..(nb_channels as usize) {
                let data = audio_frame.data(ch);
                let samples: Vec<f32> = data
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect();
                sum_squares += samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>();
                count += samples.len();
            }
            let rms = (sum_squares / count as f64).sqrt();
            let db = 20.0 * (rms.abs().log10());
            // println!("Frame dB: {}", db);
            db > threshold
        }
        // Add more formats as needed
        fmt => {
            println!("Unsupported audio format: {}", fmt);
            false
        }
    }
}

fn cut_video(input: &str, start: u32, end: u32, output: &str) -> Result<(), String> {
    ffmpeg::init().map_err(|e| e.to_string())?;
    let mut input_file = format::input(&input).map_err(|e| e.to_string())?;
    let mut start_ts = 0;
    let mut stream_mapping = HashMap::new();
    let mut ts_bounds: HashMap<usize, (i64, i64)> = HashMap::new();
    // Create output context
    let mut output_file = format::output(&output).map_err(|e| e.to_string())?;
    // Copy over streams and build index mapping
    for (idx, stream) in input_file.streams().enumerate() {
        let time_base = stream.time_base();
        start_ts = (start as i64 * time_base.denominator() as i64) / time_base.numerator() as i64;
        let end_ts = (end as i64 * time_base.denominator() as i64) / time_base.numerator() as i64; 
        ts_bounds.insert(idx, (start_ts, end_ts));
        let codec_params = stream.parameters();
        let mut out_stream = output_file.add_stream(codec_params.id()).map_err(|e| e.to_string())?;
        out_stream.set_parameters(codec_params);
        stream_mapping.insert(idx, out_stream.index());
    }
    output_file.write_header().map_err(|e| e.to_string())?;
    input_file.seek(start_ts, ..start_ts).map_err(|e| e.to_string())?;
    let mut packets_written = 0;
    let mut video_packets_written = 0;
    let mut seen_keyframe = false;
    for (stream, mut packet) in input_file.packets() {
        let stream_index = packet.stream();
        if stream.parameters().medium() == ffmpeg::media::Type::Video {
            if !seen_keyframe {
                if packet.is_key() {
                    seen_keyframe = true;
                } else {
                    continue;
                }
            }
            video_packets_written += 1;
        }
        if let Some((start_ts, end_ts)) = ts_bounds.get(&stream_index) {
            if let Some(pts) = packet.pts() {
                if pts < *start_ts || pts > *end_ts {
                    continue;
                }
            }
            let out_index = *stream_mapping.get(&stream_index).expect("Stream mapping not found");
            let out_stream_temp = output_file.stream(out_index).expect("Output stream not found");
            // Print stream type
            let medium = stream.parameters().medium();
            // println!("Writing packet for stream {} ({:?})", stream_index, medium);
            packet.set_stream(out_index);
            packet.rescale_ts(stream.time_base(), out_stream_temp.time_base());
            packet.write_interleaved(&mut output_file).map_err(|e| e.to_string())?;
            packets_written += 1;
        }
    }
    // println!("Packets written: {}", packets_written);
    if video_packets_written == 0 {
        std::fs::remove_file(output).ok();
        return Err("No video packets written for this segment (likely no keyframe in interval)".to_string());
    }
    output_file.write_trailer().map_err(|e| e.to_string())?;
    Ok(())
}

fn find_next_keyframe(input: &str, start_sec: u32) -> Option<u32> {
    ffmpeg::init().ok()?;
    let mut input_file = format::input(input).ok()?;
    // Collect video stream indices and time bases first
    let video_streams: Vec<_> = input_file.streams()
        .enumerate()
        .filter(|(_, stream)| stream.parameters().medium() == ffmpeg::media::Type::Video)
        .map(|(idx, stream)| (idx, stream.time_base()))
        .collect();

    for (stream_index, time_base) in video_streams {
        let start_ts = (start_sec as i64 * time_base.denominator() as i64) / time_base.numerator() as i64;
        for (packet_stream, packet) in input_file.packets() {
            if packet_stream.index() == stream_index {
                if let Some(pts) = packet.pts() {
                    if pts >= start_ts && packet.is_key() {
                        // Convert pts back to seconds
                        let sec = (pts * time_base.numerator() as i64 / time_base.denominator() as i64) as u32;
                        return Some(sec);
                    }
                }
            }
        }
    }
    None
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
        Commands::Cut {
            input,
            start,
            end,
            output,
        } => {
            println!(
                "Cutting from {} ({}s to {}s) -> {}",
                input, start, end, output
            );
            cut_video(&input, start, end, &output)
                .unwrap_or_else(|err| println!("Error cutting video: {}", err));
        }
        Commands::RemoveSilence { input, threshold, output } => {
            println!(
                "Removing silence from {} with threshold {} -> {}",
                input, threshold, output
            );
            cut_noisy_segments(&input, threshold, &output)
                .unwrap_or_else(|err| println!("Error removing silence: {}", err));
        }
    }
}
