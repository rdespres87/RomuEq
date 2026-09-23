use hound::{WavReader, WavWriter};
use romueq_common::EqParameters;
use romueq_engine::EngineLifecycle;
use std::env;
use std::path::PathBuf;
use std::time::Instant;

// ---------------------------------------------------------------------------
// CLI argument parsing (manual, no extra dependency)
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct CliArgs {
    command: Command,
}

#[derive(Debug)]
enum Command {
    Process {
        input: PathBuf,
        output: PathBuf,
        sample_rate: f64,
        preset: Option<Preset>,
    },
    Preset {
        command: PresetCommand,
    },
    Help,
}

#[derive(Debug)]
enum Preset {
    Load(PathBuf),
}

#[derive(Debug)]
enum PresetCommand {
    List,
    Show(String),
}

const USAGE: &str = "\
RomuEq CLI — offline audio renderer

Usage: romueq-cli <command> [options]

Commands:
  process <input.wav> <output.wav>  Process an audio file with EQ
  preset list                       List available presets
  preset show <name>                Show preset parameters
  help                              Show this help message

Options (for process):
  --sample-rate <rate>              Sample rate in Hz (default: 44100)
  --preset <name|path>              Apply preset (default: default)

Presets:
  default     Flat EQ (no processing)
  bass_boost  +6 dB at 100 Hz
  vocal       +4 dB at 1 kHz, -3 dB at 200 Hz
  de_ess      -6 dB at 5 kHz
";

macro_rules! err {
    ($($arg:tt)*) => { eprintln!($($arg)*) };
}

fn print_usage() {
    println!("{USAGE}");
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(0);
    }

    match args[1].as_str() {
        "help" | "--help" | "-h" => CliArgs {
            command: Command::Help,
        },

        "process" => {
            if args.len() < 4 {
                err!("Error: 'process' requires <input> <output>");
                std::process::exit(1);
            }

            let input = PathBuf::from(&args[2]);
            let output = PathBuf::from(&args[3]);

            let mut sample_rate: f64 = 44100.0;
            let mut preset: Option<Preset> = None;

            let mut i = 4;
            while i < args.len() {
                match args[i].as_str() {
                    "--sample-rate" => {
                        i += 1;
                        if i < args.len() {
                            sample_rate = args[i].parse().unwrap_or_else(|_| {
                                err!("Error: invalid sample rate '{}'", args[i]);
                                std::process::exit(1);
                            });
                        } else {
                            err!("Error: --sample-rate requires a value");
                            std::process::exit(1);
                        }
                    }
                    "--preset" => {
                        i += 1;
                        if i < args.len() {
                            preset = Some(Preset::Load(PathBuf::from(&args[i])));
                        } else {
                            err!("Error: --preset requires a value");
                            std::process::exit(1);
                        }
                    }
                    _ => {
                        err!("Error: unknown option '{}'", args[i]);
                        std::process::exit(1);
                    }
                }
                i += 1;
            }

            CliArgs {
                command: Command::Process {
                    input,
                    output,
                    sample_rate,
                    preset,
                },
            }
        }

        "preset" => {
            if args.len() < 3 {
                err!("Error: 'preset' requires a subcommand (list|show)");
                std::process::exit(1);
            }

            let preset_cmd = match args[2].as_str() {
                "list" => PresetCommand::List,
                "show" => {
                    if args.len() < 4 {
                        err!("Error: 'preset show' requires a preset name");
                        std::process::exit(1);
                    }
                    PresetCommand::Show(args[3].clone())
                }
                other => {
                    err!("Error: unknown preset subcommand '{other}'");
                    std::process::exit(1);
                }
            };

            CliArgs {
                command: Command::Preset {
                    command: preset_cmd,
                },
            }
        }

        other => {
            err!("Error: unknown command '{other}'");
            print_usage();
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// Preset definitions
// ---------------------------------------------------------------------------

fn default_preset() -> EqParameters {
    let mut params = EqParameters::default();
    // Disable all bands (flat EQ)
    for band in &mut params.bands {
        band.enabled = false;
    }
    params
}

fn bass_boost_preset() -> EqParameters {
    let mut params = EqParameters::default();
    // Enable band 0: low-shelf at 100 Hz, +6 dB
    if let Some(band) = params.bands.get_mut(0) {
        band.enabled = true;
        band.gain_db = 6.0;
        band.frequency_hz = 100.0;
        band.q = 0.707;
    }
    params
}

fn vocal_preset() -> EqParameters {
    let mut params = EqParameters::default();
    // Band 0: low-cut at 200 Hz, -3 dB
    if let Some(band) = params.bands.get_mut(0) {
        band.enabled = true;
        band.gain_db = -3.0;
        band.frequency_hz = 200.0;
        band.q = 0.707;
    }
    // Band 1: presence at 1 kHz, +4 dB
    if let Some(band) = params.bands.get_mut(1) {
        band.enabled = true;
        band.gain_db = 4.0;
        band.frequency_hz = 1_000.0;
        band.q = 1.0;
    }
    params
}

fn de_ess_preset() -> EqParameters {
    let mut params = EqParameters::default();
    // Band 0: de-ess at 5 kHz, -6 dB
    if let Some(band) = params.bands.get_mut(0) {
        band.enabled = true;
        band.gain_db = -6.0;
        band.frequency_hz = 5_000.0;
        band.q = 2.0;
    }
    params
}

fn get_preset(name: &str) -> EqParameters {
    match name {
        "default" => default_preset(),
        "bass_boost" => bass_boost_preset(),
        "vocal" => vocal_preset(),
        "de_ess" => de_ess_preset(),
        _ => {
            err!("Unknown preset: '{name}'");
            err!("Available presets: default, bass_boost, vocal, de_ess");
            std::process::exit(1);
        }
    }
}

fn list_presets() {
    println!("Available presets:");
    println!("  default     Flat EQ (no processing)");
    println!("  bass_boost  +6 dB at 100 Hz");
    println!("  vocal       +4 dB at 1 kHz, -3 dB at 200 Hz");
    println!("  de_ess      -6 dB at 5 kHz");
}

fn show_preset(name: &str) {
    let params = get_preset(name);
    println!("Preset: {name}");
    println!();
    for (i, band) in params.bands.iter().enumerate() {
        if band.enabled {
            println!(
                "  Band {}: enabled, f={} Hz, gain={} dB, Q={}",
                i, band.frequency_hz, band.gain_db, band.q
            );
        }
    }
    println!(
        "  Global: bypass={}, output_gain={} dB",
        params.global.bypass, params.global.output_gain_db
    );
}

// ---------------------------------------------------------------------------
// Audio processing
// ---------------------------------------------------------------------------

fn process_file(
    input_path: &PathBuf,
    output_path: &PathBuf,
    sample_rate: f64,
    eq_params: EqParameters,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read input WAV
    let mut reader = WavReader::open(input_path)?;
    let spec = reader.spec();

    if spec.channels == 0 {
        return Err("Input WAV has no channels".into());
    }

    let channels = spec.channels as usize;
    let actual_sample_rate = spec.sample_rate as f64;

    // Use the sample rate from the file (or override via CLI)
    let effective_sample_rate = if (sample_rate - 44100.0).abs() < 0.01 {
        actual_sample_rate
    } else {
        sample_rate
    };

    println!(
        "Reading input: {} ({} Hz, {} ch)",
        input_path.display(),
        actual_sample_rate,
        channels
    );

    // Read all samples into an interleaved buffer, then transpose to planar
    let total_samples = reader.len() as usize / channels;
    println!("Total samples: {total_samples}");

    let interleaved: Vec<i16> = reader.samples::<i16>().filter_map(|s| s.ok()).collect();

    // Transpose interleaved -> planar
    let samples: Vec<Vec<f32>> = (0..channels)
        .map(|ch| {
            interleaved
                .iter()
                .skip(ch)
                .step_by(channels)
                .map(|&s| s as f32 / 32768.0)
                .collect()
        })
        .collect();

    // Create and prepare the engine
    let mut engine = EngineLifecycle::new();
    if !engine.prepare(effective_sample_rate, 512, channels) {
        return Err("Failed to prepare engine".into());
    }

    // Set EQ parameters
    engine.set_parameters(eq_params);

    println!("Processing...");
    let start = Instant::now();

    // Process in blocks using a temporary output buffer per block
    const BLOCK_SIZE: usize = 512;
    let mut output_buffers: Vec<Vec<f32>> =
        (0..channels).map(|_| vec![0.0; total_samples]).collect();

    for block_start in (0..total_samples).step_by(BLOCK_SIZE) {
        let block_size = std::cmp::min(BLOCK_SIZE, total_samples - block_start);

        // Build input slices from planar buffers
        let input_slices: Vec<&[f32]> = (0..channels)
            .map(|ch| &samples[ch][block_start..block_start + block_size])
            .collect();

        // Allocate a flat output buffer for this block, then split into planar slices
        let mut block_output = vec![0.0; channels * block_size];

        // Build output slices from the flat buffer using raw pointers
        let mut block_output_slices: Vec<&mut [f32]> = (0..channels)
            .map(|ch| {
                let start = ch * block_size;
                let ptr = unsafe { block_output.as_mut_ptr().add(start) };
                // Safety: ptr is valid for `block_size` elements; block_output outlives the slices
                unsafe { std::slice::from_raw_parts_mut(ptr, block_size) }
            })
            .collect();

        // Pass slices to engine
        let success = engine.process(&input_slices, &mut block_output_slices);
        if !success {
            err!("Engine process returned false at block {block_start}");
        }

        // Copy the processed block into the output buffer
        for ch in 0..channels {
            let dest = &mut output_buffers[ch][block_start..block_start + block_size];
            for i in 0..block_size {
                dest[i] = block_output[ch * block_size + i];
            }
        }
    }

    let elapsed = start.elapsed();
    println!(
        "Processed {} samples in {:.2} ms",
        total_samples,
        elapsed.as_secs_f64() * 1000.0
    );

    // Write output WAV
    let out_spec = hound::WavSpec {
        channels: channels as u16,
        sample_rate: spec.sample_rate, // Preserve original sample rate for output
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(output_path, out_spec)?;

    for ch_data in output_buffers.iter() {
        for &sample in ch_data.iter().take(total_samples) {
            let clamped = sample.clamp(-1.0, 1.0);
            let i16_val = (clamped * 32767.0) as i16;
            writer.write_sample(i16_val)?;
        }
    }

    writer.finalize()?;
    println!("Output written to: {}", output_path.display());

    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args = parse_args();

    match args.command {
        Command::Help => {
            print_usage();
        }

        Command::Process {
            input,
            output,
            sample_rate,
            preset,
        } => {
            if !input.exists() {
                err!("Error: input file '{}' does not exist", input.display());
                std::process::exit(1);
            }

            let eq_params = match preset {
                Some(Preset::Load(path)) => {
                    // For now, treat preset path as a preset name
                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "default".to_string());
                    get_preset(&name)
                }
                None => default_preset(),
            };

            if let Err(e) = process_file(&input, &output, sample_rate, eq_params) {
                err!("Error: {e}");
                std::process::exit(1);
            }
        }

        Command::Preset { command } => match command {
            PresetCommand::List => list_presets(),
            PresetCommand::Show(name) => show_preset(&name),
        },
    }
}
