use super::{json_string, Mode, Options};
use paimon_touch_helper::codes::*;
use paimon_touch_helper::{Command, Event, Gate, Stream, SYN_REPORT};
use std::{
    error::Error,
    ffi::{c_char, c_void, CStr, CString},
    io::{self, Write},
    time::{Duration, Instant},
};

#[repr(C)]
struct Caps {
    name: [c_char; 80],
    slots: i32,
    max_x: i32,
    max_y: i32,
    max_id: i32,
    selected: i32,
    tool_finger: i32,
    axes: [i32; 64],
}

extern "C" {
    fn ph_init(supervisor: i32) -> i32;
    fn ph_open(path: *const c_char, caps: *mut Caps, device: *mut *mut c_void) -> i32;
    fn ph_create(device: *mut c_void) -> i32;
    fn ph_describe(device: *mut c_void);
    fn ph_grab(device: *mut c_void, caps: *mut Caps) -> i32;
    fn ph_seed(device: *mut c_void, caps: *const Caps) -> i32;
    fn ph_handover_ready(device: *mut c_void, caps: *const Caps) -> i32;
    fn ph_poll(device: *mut c_void, stdin: i32) -> i32;
    fn ph_read(device: *mut c_void, event: *mut Event) -> i32;
    fn ph_write(device: *mut c_void, events: *const Event, count: i32) -> i32;
    fn ph_stdin(buffer: *mut u8, capacity: i32) -> i32;
    fn ph_close(device: *mut c_void) -> i32;
}

fn check(result: i32) -> Result<i32, Box<dyn Error>> {
    if result < 0 {
        Err(io::Error::from_raw_os_error(-result).into())
    } else {
        Ok(result)
    }
}

struct Device(*mut c_void);
impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            ph_close(self.0);
        }
    }
}
impl Device {
    fn close(mut self) -> Result<(), Box<dyn Error>> {
        let pointer = std::mem::replace(&mut self.0, std::ptr::null_mut());
        check(unsafe { ph_close(pointer) })?;
        Ok(())
    }
    fn open(path: &str) -> Result<(Self, Caps), Box<dyn Error>> {
        let path = CString::new(path)?;
        // All fields are plain integers; C fills name, ranges and all 64 axes.
        let mut caps: Caps = unsafe { std::mem::zeroed() };
        let mut pointer = std::ptr::null_mut();
        check(unsafe { ph_open(path.as_ptr(), &mut caps, &mut pointer) })?;
        Ok((Self(pointer), caps))
    }
    fn write(&self, frame: &[Event]) -> Result<(), Box<dyn Error>> {
        if !frame.is_empty() {
            check(unsafe { ph_write(self.0, frame.as_ptr(), frame.len() as i32) })?;
        }
        Ok(())
    }
}

fn heartbeat() -> Result<(), Box<dyn Error>> {
    println!("{{\"event\":\"heartbeat\"}}");
    io::stdout().flush()?;
    Ok(())
}

fn report_caps(path: &str, caps: &Caps) {
    let name = unsafe { CStr::from_ptr(caps.name.as_ptr()) }.to_string_lossy();
    println!("{{\"event\":\"capabilities\",\"path\":{},\"name\":{},\"slots\":{},\"rawMaxX\":{},\"rawMaxY\":{},\"trackingMax\":{}}}",
        json_string(path), json_string(&name), caps.slots, caps.max_x, caps.max_y, caps.max_id);
}

pub fn run(options: Options) -> Result<i32, Box<dyn Error>> {
    let parent: i32 = std::env::var("PAIMON_SUPERVISOR_PID")?.parse()?;
    check(unsafe { ph_init(parent) })?;
    heartbeat()?;
    if options.mode == Mode::Inspect && options.device.is_none() {
        let mut found = 0;
        for entry in std::fs::read_dir("/dev/input")? {
            heartbeat()?;
            let path = entry?.path();
            if !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("event"))
            {
                continue;
            }
            let path = path.to_string_lossy();
            if let Ok((_device, caps)) = Device::open(&path) {
                report_caps(&path, &caps);
                unsafe {
                    ph_describe(_device.0);
                }
                found += 1;
            }
        }
        if found == 0 {
            return Err("no supported physical touchscreen found".into());
        }
        return Ok(0);
    }
    let path = options.device.as_deref().ok_or("device required")?;
    let (device, mut caps) = Device::open(path)?;
    report_caps(path, &caps);
    unsafe {
        ph_describe(device.0);
    }
    if options.mode == Mode::Inspect {
        return Ok(0);
    }
    if options.mode != Mode::Grab {
        check(unsafe { ph_create(device.0) })?;
        println!(
            "{{\"event\":\"created\",\"sentTouch\":false,\"displayAssociation\":\"unverified\"}}"
        );
    }
    let active = options.mode.forwards();
    if options.mode != Mode::Create {
        // Allow Android to enumerate the empty virtual device before takeover.
        if active {
            for _ in 0..30 {
                heartbeat()?;
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        check(unsafe { ph_grab(device.0, &mut caps) })?;
        println!("{{\"event\":\"grabbed\",\"exclusive\":true}}");
    }
    let mut stream = Stream::new(caps.slots as usize, caps.max_x, caps.max_y, caps.max_id)?;
    if caps.tool_finger != 0 {
        stream = stream.with_tool_finger();
    }
    if active {
        check(unsafe { ph_seed(device.0, &caps) })?;
        stream.forward(&[Event::new(EV_ABS, ABS_MT_SLOT, caps.selected), SYN_REPORT])?;
    }
    let start = Instant::now();
    // Start draining before the supervisor's hard deadline; never extend that deadline.
    let soft_limit = Duration::from_secs(options.seconds).saturating_sub(Duration::from_secs(1));
    let mut gate = Gate::default();
    let mut frame = Vec::new();
    let mut frame_started = None;
    let mut commands = Vec::new();
    let mut stdin_open = true;
    let mut next_heartbeat = Instant::now();
    let mut tap_until = None;
    let mut next_tap = 0;
    loop {
        let now_ms = start.elapsed().as_millis() as u64;
        if start.elapsed() >= soft_limit {
            gate.apply(Command::Stop, now_ms);
        }
        gate.expire(now_ms);
        let ready = check(unsafe { ph_poll(device.0, i32::from(stdin_open)) })?;
        if ready & 4 != 0 {
            gate.apply(Command::Stop, now_ms);
        }
        // The single writer processes physical events first, with bounded work per iteration.
        if (active || options.mode == Mode::Grab) && ready & 1 != 0 {
            for _ in 0..256 {
                let mut event = Event::new(0, 0, 0);
                if check(unsafe { ph_read(device.0, &mut event) })? == 0 {
                    break;
                }
                // A grab-only probe consumes history without ever writing touch.
                if !active {
                    continue;
                }
                if event.kind == EV_SYN && event.code == SYN_DROPPED {
                    return Err("SYN_DROPPED: state lost; abnormal recovery".into());
                }
                frame.push(event);
                frame_started.get_or_insert_with(Instant::now);
                if frame.len() > 4096 {
                    return Err("physical frame exceeds 4096 events".into());
                }
                if event == SYN_REPORT {
                    device.write(&stream.forward(&frame)?)?;
                    frame.clear();
                    frame_started = None;
                }
            }
        }
        if frame_started
            .is_some_and(|time| Instant::now().duration_since(time) > Duration::from_millis(250))
        {
            return Err("incomplete physical frame exceeded 250ms".into());
        }
        if ready & 2 != 0 {
            let mut buffer = [0u8; 256];
            let count = unsafe { ph_stdin(buffer.as_mut_ptr(), buffer.len() as i32) };
            if count == 0 {
                stdin_open = false;
                gate.apply(Command::Stop, now_ms);
            } else if count != -11 {
                let count = check(count)? as usize;
                commands.extend_from_slice(&buffer[..count]);
                if commands.len() > 512 {
                    gate.apply(Command::Stop, now_ms);
                    stdin_open = false;
                    commands.clear();
                }
            }
        }
        // Never splice a synthetic frame into a partially received physical frame.
        if frame.is_empty() {
            if gate.draining() || tap_until.is_some_and(|until| now_ms >= until) {
                device.write(&stream.end_tap())?;
                tap_until = None;
            }
            while let Some(end) = commands.iter().position(|c| *c == b'\n') {
                let line: Vec<_> = commands.drain(..=end).collect();
                let parsed = std::str::from_utf8(&line)
                    .ok()
                    .and_then(|line| Command::parse(line).ok());
                match parsed {
                    Some(Command::Tap(x, y))
                        if options.mode == Mode::Merge
                            && now_ms >= next_tap
                            && gate.allow_tap(now_ms) =>
                    {
                        match stream.begin_tap(x, y) {
                            Ok(mut output) => {
                                let extra: Vec<_> = (ABS_MT_TOUCH_MAJOR..=ABS_MT_TOOL_Y)
                                    .filter(|code| {
                                        ![ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_TRACKING_ID]
                                            .contains(code)
                                            && caps.axes[*code as usize] != i32::MIN
                                    })
                                    .map(|code| {
                                        Event::new(
                                            EV_ABS,
                                            code,
                                            if code == ABS_MT_TOOL_X {
                                                x
                                            } else if code == ABS_MT_TOOL_Y {
                                                y
                                            } else {
                                                caps.axes[code as usize]
                                            },
                                        )
                                    })
                                    .collect();
                                output.splice(4..4, extra);
                                device.write(&output)?;
                                gate.tap_started();
                                tap_until = Some(now_ms + 50);
                                next_tap = now_ms + 1000;
                                println!("{{\"event\":\"tap_started\",\"rawX\":{x},\"rawY\":{y}}}");
                            }
                            Err(reason) => println!(
                                "{{\"event\":\"tap_rejected\",\"reason\":{}}}",
                                json_string(reason)
                            ),
                        }
                    }
                    Some(command @ (Command::Permit | Command::Stop)) => {
                        gate.apply(command, now_ms)
                    }
                    Some(Command::Tap(_, _)) => println!(
                        "{{\"event\":\"tap_rejected\",\"reason\":\"mode, permit, or rate limit\"}}"
                    ),
                    None => {
                        gate.apply(Command::Stop, now_ms);
                        println!("{{\"event\":\"draining\",\"reason\":\"invalid command\"}}");
                    }
                }
            }
            if gate.draining()
                && stream.real_fingers_up()
                && (options.mode == Mode::Create
                    || check(unsafe { ph_handover_ready(device.0, &caps) })? == 1)
            {
                device.write(&stream.end_tap())?;
                break;
            }
        }
        if Instant::now() >= next_heartbeat {
            heartbeat()?;
            next_heartbeat = Instant::now() + Duration::from_millis(100);
        }
        if !active {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    device.close()?;
    println!("{{\"event\":\"closed\",\"coexistence\":\"unverified\"}}");
    Ok(0)
}
