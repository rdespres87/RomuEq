//! Command-line argument parsing and command dispatch for `equaform-cli`.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{default_preset, get_preset, list_preset_names, process_file};

/// CLI usage text printed by `help` and when invoked without arguments.
pub const USAGE: &str = "\
Equaform CLI — offline audio renderer

Usage: equaform-cli <command> [options]

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

/// Parsed top-level CLI invocation.
#[derive(Debug, PartialEq)]
pub struct CliArgs {
    pub command: Command,
}

/// Top-level CLI subcommand.
#[derive(Debug, PartialEq)]
pub enum Command {
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

/// Subcommands under `preset`.
#[derive(Debug, PartialEq, Eq)]
pub enum PresetCommand {
    List,
    Show(String),
}

/// Error produced while parsing CLI arguments.
#[derive(Debug, PartialEq, Eq)]
pub struct CliError {
    pub message: String,
    pub show_usage: bool,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            show_usage: false,
        }
    }

    fn with_usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            show_usage: true,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

struct ProcessOptions {
    sample_rate: f64,
    preset: Option<String>,
}

/// Parse CLI arguments (excluding the program binary name).
pub fn parse_args<I, S>(args: I) -> Result<CliArgs, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut iter = args.into_iter();
    let Some(first) = iter.next() else {
        return Ok(CliArgs {
            command: Command::Help,
        });
    };

    match first.as_ref() {
        "help" | "--help" | "-h" => Ok(CliArgs {
            command: Command::Help,
        }),
        "process" => parse_process_args(iter),
        "preset" => parse_preset_args(iter),
        other => Err(CliError::with_usage(format!(
            "Error: unknown command '{other}'"
        ))),
    }
}

fn parse_process_args<I, S>(mut iter: I) -> Result<CliArgs, CliError>
where
    I: Iterator<Item = S>,
    S: AsRef<str>,
{
    let Some(input) = iter.next() else {
        return Err(CliError::new("Error: 'process' requires <input> <output>"));
    };
    let Some(output) = iter.next() else {
        return Err(CliError::new("Error: 'process' requires <input> <output>"));
    };

    let mut opts = ProcessOptions {
        sample_rate: 44100.0,
        preset: None,
    };

    while let Some(arg) = iter.next() {
        parse_process_option(arg.as_ref(), &mut iter, &mut opts)?;
    }

    Ok(CliArgs {
        command: Command::Process {
            input: PathBuf::from(input.as_ref()),
            output: PathBuf::from(output.as_ref()),
            sample_rate: opts.sample_rate,
            preset: opts.preset,
        },
    })
}

fn parse_process_option<I, S>(
    flag: &str,
    iter: &mut I,
    opts: &mut ProcessOptions,
) -> Result<(), CliError>
where
    I: Iterator<Item = S>,
    S: AsRef<str>,
{
    match flag {
        "--sample-rate" => {
            let val = iter
                .next()
                .ok_or_else(|| CliError::new("Error: --sample-rate requires a value"))?;
            let val_str = val.as_ref();
            opts.sample_rate = val_str
                .parse()
                .map_err(|_| CliError::new(format!("Error: invalid sample rate '{val_str}'")))?;
            Ok(())
        }
        "--preset" => {
            let val = iter
                .next()
                .ok_or_else(|| CliError::new("Error: --preset requires a value"))?;
            opts.preset = Some(val.as_ref().to_string());
            Ok(())
        }
        other => Err(CliError::new(format!("Error: unknown option '{other}'"))),
    }
}

fn parse_preset_args<I, S>(mut iter: I) -> Result<CliArgs, CliError>
where
    I: Iterator<Item = S>,
    S: AsRef<str>,
{
    let Some(sub) = iter.next() else {
        return Err(CliError::new(
            "Error: 'preset' requires a subcommand (list|show)",
        ));
    };

    let preset_cmd = match sub.as_ref() {
        "list" => PresetCommand::List,
        "show" => {
            let name = iter
                .next()
                .ok_or_else(|| CliError::new("Error: 'preset show' requires a preset name"))?;
            PresetCommand::Show(name.as_ref().to_string())
        }
        other => {
            return Err(CliError::new(format!(
                "Error: unknown preset subcommand '{other}'"
            )));
        }
    };

    Ok(CliArgs {
        command: Command::Preset {
            command: preset_cmd,
        },
    })
}

/// Parse arguments and execute the resulting command, returning the process exit code.
pub fn run_cli<I, S, Out, ErrW>(args: I, stdout: &mut Out, stderr: &mut ErrW) -> i32
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    Out: Write,
    ErrW: Write,
{
    match parse_args(args) {
        Ok(parsed) => dispatch(parsed.command, stdout, stderr),
        Err(err) => {
            let _ = writeln!(stderr, "{err}");
            if err.show_usage {
                let _ = writeln!(stdout, "{USAGE}");
            }
            1
        }
    }
}

/// Dispatch a parsed CLI command, writing output to the provided streams and returning an exit code.
pub fn dispatch<Out: Write, ErrW: Write>(
    command: Command,
    stdout: &mut Out,
    stderr: &mut ErrW,
) -> i32 {
    match command {
        Command::Help => {
            let _ = writeln!(stdout, "{USAGE}");
            0
        }
        Command::Process {
            input,
            output,
            sample_rate,
            preset,
        } => dispatch_process(&input, &output, sample_rate, preset.as_deref(), stderr),
        Command::Preset { command } => dispatch_preset(command, stdout, stderr),
    }
}

fn dispatch_process<ErrW: Write>(
    input: &Path,
    output: &Path,
    sample_rate: f64,
    preset: Option<&str>,
    stderr: &mut ErrW,
) -> i32 {
    if !input.exists() {
        let _ = writeln!(
            stderr,
            "Error: input file '{}' does not exist",
            input.display()
        );
        return 1;
    }

    let eq_params = match preset {
        Some(name) => match get_preset(name) {
            Some(params) => params,
            None => {
                let _ = writeln!(stderr, "Unknown preset: '{name}'");
                let _ = writeln!(
                    stderr,
                    "Available presets: {}",
                    list_preset_names().join(", ")
                );
                return 1;
            }
        },
        None => default_preset(),
    };

    let input_buf = input.to_path_buf();
    let output_buf = output.to_path_buf();
    if let Err(e) = process_file(&input_buf, &output_buf, sample_rate, eq_params) {
        let _ = writeln!(stderr, "Error: {e}");
        return 1;
    }

    0
}

fn dispatch_preset<Out: Write, ErrW: Write>(
    command: PresetCommand,
    stdout: &mut Out,
    stderr: &mut ErrW,
) -> i32 {
    match command {
        PresetCommand::List => {
            let _ = writeln!(stdout, "Available presets:");
            for name in list_preset_names() {
                let _ = writeln!(stdout, "  {name}");
            }
            0
        }
        PresetCommand::Show(name) => {
            let Some(params) = get_preset(&name) else {
                let _ = writeln!(stderr, "Unknown preset: '{name}'");
                return 1;
            };
            let _ = writeln!(stdout, "Preset: {name}");
            let _ = writeln!(stdout);
            for (i, band) in params.bands.iter().enumerate() {
                if band.enabled {
                    let _ = writeln!(
                        stdout,
                        "  Band {}: enabled, f={} Hz, gain={} dB, Q={}",
                        i, band.frequency_hz, band.gain_db, band.q
                    );
                }
            }
            let _ = writeln!(
                stdout,
                "  Global: bypass={}, output_gain={} dB",
                params.global.bypass, params.global.output_gain_db
            );
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
    use std::sync::atomic::{AtomicU64, Ordering};

    fn unique_temp_wav(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "equaform_cli_{tag}_{}_{id}.wav",
            std::process::id()
        ))
    }

    fn write_test_wav(path: &Path, channels: u16, sample_rate: u32, frames: usize) {
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(path, spec).expect("create test wav");
        for i in 0..frames * channels as usize {
            let sample = ((i as f32 * 0.05).sin() * 10000.0) as i16;
            writer.write_sample(sample).expect("write test sample");
        }
        writer.finalize().expect("finalize test wav");
    }

    #[test]
    fn parse_empty_and_help_variants() {
        for argv in [vec![], vec!["help"], vec!["--help"], vec!["-h"]] {
            let parsed = parse_args(argv).expect("help args should parse");
            assert_eq!(parsed.command, Command::Help);
        }
    }

    #[test]
    fn parse_process_defaults_and_options() {
        let parsed = parse_args(["process", "in.wav", "out.wav"]).expect("process defaults");
        assert_eq!(
            parsed.command,
            Command::Process {
                input: PathBuf::from("in.wav"),
                output: PathBuf::from("out.wav"),
                sample_rate: 44100.0,
                preset: None,
            }
        );

        let parsed = parse_args([
            "process",
            "in.wav",
            "out.wav",
            "--sample-rate",
            "48000",
            "--preset",
            "vocal",
        ])
        .expect("process with options");
        assert_eq!(
            parsed.command,
            Command::Process {
                input: PathBuf::from("in.wav"),
                output: PathBuf::from("out.wav"),
                sample_rate: 48000.0,
                preset: Some("vocal".to_string()),
            }
        );
    }

    #[test]
    fn parse_process_errors() {
        let err = parse_args(["process"]).unwrap_err();
        assert_eq!(err.message, "Error: 'process' requires <input> <output>");
        assert!(!err.show_usage);

        let err = parse_args(["process", "only_input.wav"]).unwrap_err();
        assert_eq!(err.message, "Error: 'process' requires <input> <output>");

        let err = parse_args(["process", "a.wav", "b.wav", "--sample-rate"]).unwrap_err();
        assert_eq!(err.message, "Error: --sample-rate requires a value");

        let err = parse_args(["process", "a.wav", "b.wav", "--sample-rate", "abc"]).unwrap_err();
        assert_eq!(err.message, "Error: invalid sample rate 'abc'");

        let err = parse_args(["process", "a.wav", "b.wav", "--preset"]).unwrap_err();
        assert_eq!(err.message, "Error: --preset requires a value");

        let err = parse_args(["process", "a.wav", "b.wav", "--unknown"]).unwrap_err();
        assert_eq!(err.message, "Error: unknown option '--unknown'");
    }

    #[test]
    fn parse_preset_variants_and_errors() {
        let parsed = parse_args(["preset", "list"]).expect("preset list");
        assert_eq!(
            parsed.command,
            Command::Preset {
                command: PresetCommand::List
            }
        );

        let parsed = parse_args(["preset", "show", "bass_boost"]).expect("preset show");
        assert_eq!(
            parsed.command,
            Command::Preset {
                command: PresetCommand::Show("bass_boost".to_string())
            }
        );

        let err = parse_args(["preset"]).unwrap_err();
        assert_eq!(
            err.message,
            "Error: 'preset' requires a subcommand (list|show)"
        );

        let err = parse_args(["preset", "show"]).unwrap_err();
        assert_eq!(err.message, "Error: 'preset show' requires a preset name");

        let err = parse_args(["preset", "delete"]).unwrap_err();
        assert_eq!(err.message, "Error: unknown preset subcommand 'delete'");
    }

    #[test]
    fn parse_unknown_command_shows_usage() {
        let err = parse_args(["frobnicate"]).unwrap_err();
        assert_eq!(err.message, "Error: unknown command 'frobnicate'");
        assert!(err.show_usage);
    }

    #[test]
    fn run_cli_help_and_unknown_command() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_cli(["--help"], &mut out, &mut err);
        assert_eq!(code, 0);
        assert!(err.is_empty());
        assert_eq!(String::from_utf8_lossy(&out), format!("{USAGE}\n"));

        out.clear();
        err.clear();
        let code = run_cli(["bogus"], &mut out, &mut err);
        assert_eq!(code, 1);
        assert_eq!(
            String::from_utf8_lossy(&err),
            "Error: unknown command 'bogus'\n"
        );
        assert_eq!(String::from_utf8_lossy(&out), format!("{USAGE}\n"));
    }

    #[test]
    fn run_cli_parse_error_without_usage() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_cli(
            ["process", "in.wav", "out.wav", "--bad"],
            &mut out,
            &mut err,
        );
        assert_eq!(code, 1);
        assert!(out.is_empty());
        assert_eq!(
            String::from_utf8_lossy(&err),
            "Error: unknown option '--bad'\n"
        );
    }

    #[test]
    fn run_cli_preset_list_and_show() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_cli(["preset", "list"], &mut out, &mut err);
        assert_eq!(code, 0);
        assert!(err.is_empty());
        let stdout = String::from_utf8_lossy(&out);
        assert!(stdout.contains("Available presets:"));
        assert!(stdout.contains("  default"));
        assert!(stdout.contains("  bass_boost"));
        assert!(stdout.contains("  vocal"));
        assert!(stdout.contains("  de_ess"));

        out.clear();
        err.clear();
        let code = run_cli(["preset", "show", "vocal"], &mut out, &mut err);
        assert_eq!(code, 0);
        assert!(err.is_empty());
        let stdout = String::from_utf8_lossy(&out);
        assert!(stdout.contains("Preset: vocal"));
        assert!(stdout.contains("Band 0: enabled, f=200 Hz, gain=-3 dB, Q=0.707"));
        assert!(stdout.contains("Band 1: enabled, f=1000 Hz, gain=4 dB, Q=1"));
        assert!(stdout.contains("Global: bypass=false, output_gain=0 dB"));

        out.clear();
        err.clear();
        let code = run_cli(["preset", "show", "missing"], &mut out, &mut err);
        assert_eq!(code, 1);
        assert_eq!(String::from_utf8_lossy(&err), "Unknown preset: 'missing'\n");
    }

    #[test]
    fn run_cli_process_error_paths() {
        let missing = unique_temp_wav("missing");
        let output = unique_temp_wav("missing_out");
        let mut out = Vec::new();
        let mut err = Vec::new();

        let code = run_cli(
            [
                "process",
                missing.to_str().unwrap(),
                output.to_str().unwrap(),
            ],
            &mut out,
            &mut err,
        );
        assert_eq!(code, 1);
        assert!(String::from_utf8_lossy(&err).contains("does not exist"));

        let existing = unique_temp_wav("existing");
        write_test_wav(&existing, 2, 44100, 128);
        out.clear();
        err.clear();
        let code = run_cli(
            [
                "process",
                existing.to_str().unwrap(),
                output.to_str().unwrap(),
                "--preset",
                "nope",
            ],
            &mut out,
            &mut err,
        );
        assert_eq!(code, 1);
        let stderr = String::from_utf8_lossy(&err);
        assert!(stderr.contains("Unknown preset: 'nope'"));
        assert!(stderr.contains("Available presets: default, bass_boost, vocal, de_ess"));

        // Invalid WAV content triggers process_file error branch
        let bad_wav = unique_temp_wav("bad");
        std::fs::write(&bad_wav, b"not a wav file").expect("write invalid wav");
        err.clear();
        let code = run_cli(
            [
                "process",
                bad_wav.to_str().unwrap(),
                output.to_str().unwrap(),
            ],
            &mut out,
            &mut err,
        );
        assert_eq!(code, 1);
        assert!(String::from_utf8_lossy(&err).starts_with("Error: "));

        let _ = std::fs::remove_file(existing);
        let _ = std::fs::remove_file(bad_wav);
        let _ = std::fs::remove_file(output);
    }

    #[test]
    fn run_cli_process_end_to_end_default_and_preset() {
        let input = unique_temp_wav("in");
        let output_default = unique_temp_wav("out_default");
        let output_preset = unique_temp_wav("out_preset");
        write_test_wav(&input, 2, 48000, 1024);

        let mut out = Vec::new();
        let mut err = Vec::new();

        let code = run_cli(
            [
                "process",
                input.to_str().unwrap(),
                output_default.to_str().unwrap(),
            ],
            &mut out,
            &mut err,
        );
        assert_eq!(code, 0);
        assert!(err.is_empty());

        let reader = WavReader::open(&output_default).expect("open default output");
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().sample_rate, 48000);
        assert_eq!(reader.len(), 2048);

        let code = run_cli(
            [
                "process",
                input.to_str().unwrap(),
                output_preset.to_str().unwrap(),
                "--sample-rate",
                "96000",
                "--preset",
                "bass_boost",
            ],
            &mut out,
            &mut err,
        );
        assert_eq!(code, 0);
        assert!(err.is_empty());
        assert!(output_preset.exists());

        let _ = std::fs::remove_file(input);
        let _ = std::fs::remove_file(output_default);
        let _ = std::fs::remove_file(output_preset);
    }
}
