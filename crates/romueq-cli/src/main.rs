#![forbid(unsafe_code)]

//! RomuEq CLI — offline audio renderer binary.
//!
//! Usage: romueq-cli <command> [options]
//! Commands: process, preset (list|show), help

use romueq_cli::{default_preset, get_preset, process_file};
use std::env;
use std::path::PathBuf;

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
        preset: Option<String>,
    },
    Preset {
        command: PresetCommand,
    },
    Help,
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
  --preset <name>                   Apply preset (default: default)

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
            let mut preset: Option<String> = None;

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
                            preset = Some(args[i].clone());
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
                Some(name) => get_preset(&name).unwrap_or_else(|| {
                    err!("Unknown preset: '{name}'");
                    err!(
                        "Available presets: {}",
                        romueq_cli::list_preset_names().join(", ")
                    );
                    std::process::exit(1);
                }),
                None => default_preset(),
            };

            if let Err(e) = process_file(&input, &output, sample_rate, eq_params) {
                err!("Error: {e}");
                std::process::exit(1);
            }
        }

        Command::Preset { command } => match command {
            PresetCommand::List => {
                println!("Available presets:");
                for name in romueq_cli::list_preset_names() {
                    println!("  {name}");
                }
            }
            PresetCommand::Show(name) => {
                let params = get_preset(&name).unwrap_or_else(|| {
                    err!("Unknown preset: '{name}'");
                    std::process::exit(1);
                });
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
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use romueq_cli::{bass_boost_preset, de_ess_preset, list_preset_names, vocal_preset};

    #[test]
    fn test_preset_names() {
        assert_eq!(
            list_preset_names(),
            &["default", "bass_boost", "vocal", "de_ess"]
        );
    }

    #[test]
    fn test_get_preset_exists() {
        assert!(get_preset("default").is_some());
        assert!(get_preset("bass_boost").is_some());
        assert!(get_preset("vocal").is_some());
        assert!(get_preset("de_ess").is_some());
    }

    #[test]
    fn test_get_preset_unknown() {
        assert!(get_preset("nonexistent").is_none());
    }

    #[test]
    fn test_default_preset_is_flat() {
        let params = default_preset();
        for band in &params.bands {
            assert!(!band.enabled);
        }
    }

    #[test]
    fn test_bass_boost_preset() {
        let params = bass_boost_preset();
        assert!(params.bands[0].enabled);
        assert!((params.bands[0].frequency_hz - 100.0).abs() < 0.01);
        assert!((params.bands[0].gain_db - 6.0).abs() < 0.01);
    }

    #[test]
    fn test_vocal_preset() {
        let params = vocal_preset();
        assert!(params.bands[0].enabled);
        assert!((params.bands[0].gain_db - (-3.0)).abs() < 0.01);
        assert!(params.bands[1].enabled);
        assert!((params.bands[1].gain_db - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_de_ess_preset() {
        let params = de_ess_preset();
        assert!(params.bands[0].enabled);
        assert!((params.bands[0].frequency_hz - 5000.0).abs() < 0.01);
        assert!((params.bands[0].gain_db - (-6.0)).abs() < 0.01);
    }
}
