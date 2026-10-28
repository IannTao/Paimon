#[cfg(target_os = "android")]
mod android;

use paimon_touch_helper::supervisor::{supervise, Outcome};
use std::{process::Command, time::Duration};

const USAGE: &str = "paimon-touch-helper inspect\npaimon-touch-helper MODE --device /dev/input/eventN --seconds 1..120 [--display-verified]\nMODE: inspect | create | grab | forward | merge\nforward/merge require --display-verified after create-only dumpsys verification.\nstdin: permit (1s lease), tap RAW_X RAW_Y, stop. Tap duration: 50ms; maximum: 30.\nstop/EOF/permit expiry drain until fingers lift; watchdog kills at hard deadline.";

#[derive(Clone, Debug)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
struct Options {
    mode: Mode,
    device: Option<String>,
    seconds: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Inspect,
    Create,
    Grab,
    Forward,
    Merge,
}
impl Mode {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "inspect" => Ok(Self::Inspect),
            "create" => Ok(Self::Create),
            "grab" => Ok(Self::Grab),
            "forward" => Ok(Self::Forward),
            "merge" => Ok(Self::Merge),
            _ => Err("unknown mode"),
        }
    }
    fn forwards(self) -> bool {
        matches!(self, Self::Forward | Self::Merge)
    }
}

fn parse(args: &[String]) -> Result<Options, &'static str> {
    let mode = Mode::parse(args.first().ok_or("missing mode")?)?;
    let mut device = None;
    let mut seconds = None;
    let mut display_verified = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--device" if device.is_none() => {
                index += 1;
                let path = args.get(index).ok_or("missing device")?;
                let number = path
                    .strip_prefix("/dev/input/event")
                    .ok_or("invalid evdev path")?;
                if number.is_empty() || !number.bytes().all(|c| c.is_ascii_digit()) {
                    return Err("invalid evdev path");
                }
                device = Some(path.clone());
            }
            "--seconds" if seconds.is_none() => {
                index += 1;
                let value: u64 = args
                    .get(index)
                    .ok_or("missing seconds")?
                    .parse()
                    .map_err(|_| "invalid seconds")?;
                if !(1..=120).contains(&value) {
                    return Err("seconds must be 1..120");
                }
                seconds = Some(value);
            }
            "--display-verified" if !display_verified => display_verified = true,
            _ => return Err("unknown or duplicate option"),
        }
        index += 1;
    }
    if mode != Mode::Inspect && (device.is_none() || seconds.is_none()) {
        return Err("device and seconds are required");
    }
    if mode.forwards() && !display_verified {
        return Err("verify virtual touchscreen display association first");
    }
    Ok(Options {
        mode,
        device,
        seconds: seconds.unwrap_or(5),
    })
}

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args == ["--help"] {
        println!("{USAGE}");
        return Ok(0);
    }
    let worker = args.first().is_some_and(|s| s == "--worker");
    let options = parse(if worker { &args[1..] } else { &args })?;
    if worker {
        #[cfg(target_os = "android")]
        return android::run(options);
        #[cfg(not(target_os = "android"))]
        return Err("device operations require Android arm64 (shell UID 2000)".into());
    }
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--worker")
        .args(args)
        .env("PAIMON_SUPERVISOR_PID", std::process::id().to_string());
    match supervise(
        command,
        Duration::from_secs(options.seconds),
        Duration::from_secs(2),
    )? {
        Outcome::Exited(code) => Ok(code),
        reason => {
            eprintln!("{{\"event\":\"forced_recovery\",\"reason\":\"{reason:?}\",\"continuousGesture\":false}}");
            Ok(2)
        }
    }
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!(
                "{{\"event\":\"error\",\"message\":{}}}",
                json_string(&error.to_string())
            );
            std::process::exit(1);
        }
    }
}

fn json_string(value: &str) -> String {
    let mut output = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            c if c.is_control() => output.push_str(&format!("\\u{:04x}", c as u32)),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}
