extern crate yaml_rust;

// use yaml_rust::{YamlLoader};

use std::os::unix::fs::PermissionsExt;
use std::{thread, time::Duration, path::Path, fs};
use std::os::unix::net::UnixDatagram;
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashSet;
use clap::{Parser, Subcommand};

use evdev_rs::enums::{BusType, EventCode, EventType, EV_KEY, EV_REL, EV_SYN};
use evdev_rs::{Device, DeviceWrapper, InputEvent, UInputDevice, ReadFlag, UninitDevice, TimeVal, GrabMode};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long)]
    config: Option<String>,

    #[arg(short, long)]
    device: Option<String>,

    #[arg(short, long)]
    verbose: bool,
}


#[derive(Subcommand)]
enum Commands {
    Enable,
    Disable
}

fn print_devices() {
    let mut devices = evdev::enumerate().map(|t| t.1).collect::<Vec<_>>();
    // readdir returns them in reverse order from their eventN names for some reason
    devices.reverse();

    for (i, d) in devices.iter().enumerate() {
        println!("{}: {}", i, d.name().unwrap_or("Unnamed device"));
    }
}

fn detect_directions(keycodes: &HashSet<EventCode>, up_key: &EventCode, down_key: &EventCode, left_key: &EventCode, right_key: &EventCode) -> (bool, bool, bool, bool) {
    let mut up = false;
    let mut down = false;
    let mut left = false;
    let mut right = false;

    for keycode in keycodes {
        match keycode {
            k if k == up_key => up = true,
            k if k == down_key => down = true,
            k if k == left_key => left = true,
            k if k == right_key => right = true,
            _ => (),
        }
    }

    (up, down, left, right)
}

fn detect_mouse(keycodes: &HashSet<EventCode>, left_button: &EventCode, right_button: &EventCode, middle_button: &EventCode) -> (bool, bool, bool) {
    let mut one = false;
    let mut two = false;
    let mut three = false;

    for keycode in keycodes {
        match keycode {
            k if k == left_button => one = true,
            k if k == right_button => two = true,
            k if k == middle_button => three = true,
            _ => (),
        }
    }
    (one, two, three)
}

fn init_uinput_device() -> std::io::Result<UInputDevice> {
    // Create virtual device
    let u = UninitDevice::new().unwrap();

    // Setup device
    // per: https://01.org/linuxgraphics/gfx-docs/drm/input/uinput.html#mouse-movements

    u.set_name("Virtual Mouse");
    u.set_bustype(BusType::BUS_USB as u16);
    u.set_vendor_id(0xabcd);
    u.set_product_id(0xefef);

    // Note mouse keys have to be enabled for this to be detected
    // as a usable device, see: https://stackoverflow.com/a/64559658/6074942
    u.enable_event_type(&EventType::EV_KEY)?;
    u.enable_event_code(&EventCode::EV_KEY(EV_KEY::BTN_LEFT), None)?;
    u.enable_event_code(&EventCode::EV_KEY(EV_KEY::BTN_RIGHT), None)?;

    u.enable_event_type(&EventType::EV_REL)?;
    u.enable_event_code(&EventCode::EV_REL(EV_REL::REL_X), None)?;
    u.enable_event_code(&EventCode::EV_REL(EV_REL::REL_Y), None)?;

    u.enable_event_code(&EventCode::EV_SYN(EV_SYN::SYN_REPORT), None)?;

    // Attempt to create UInputDevice from UninitDevice
    return UInputDevice::create_from_device(&u);
}

fn get_timeval() -> TimeVal {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    TimeVal {
        tv_sec: duration.as_secs() as i64,
        tv_usec: duration.subsec_micros() as i64,
    }
}

fn write_x_input_event(device: &UInputDevice, value: i32) -> std::io::Result<()> {
    let event = InputEvent {
        time: get_timeval(),
        event_code: EventCode::EV_REL(EV_REL::REL_X),
        value: value,
    };

    write_event(device, event);
    Ok(())
}

fn write_y_input_event(device: &UInputDevice, value: i32) -> std::io::Result<()> {
    let event = InputEvent {
        time: get_timeval(),
        event_code: EventCode::EV_REL(EV_REL::REL_Y),
        value: value,
    };

    write_event(device, event);
    Ok(())
}

fn write_btn_event(device: &UInputDevice, left: bool, middle: bool, right: bool) -> std::io::Result<()> {
    write_event(device, 
        InputEvent {
            time: get_timeval(),
            event_code: EventCode::EV_KEY(EV_KEY::BTN_LEFT),
            value: if left { 1 } else { 0 },
        }
    );

    write_event(device, 
        InputEvent {
            time: get_timeval(),
            event_code: EventCode::EV_KEY(EV_KEY::BTN_RIGHT),
            value: if right { 1 } else { 0 },
        }
    );

    write_event(device, 
        InputEvent {
            time: get_timeval(),
            event_code: EventCode::EV_KEY(EV_KEY::BTN_MIDDLE),
            value: if middle { 1 } else { 0 },
        }
    );

    Ok(())
}

fn write_syn(device: &UInputDevice) -> std::io::Result<()> {
    let event = InputEvent {
        time: get_timeval(),
        event_code: EventCode::EV_SYN(EV_SYN::SYN_REPORT),
        value: 0,
    };

    device.write_event(&event)?;
    Ok(())
}

fn write_event(device: &UInputDevice, event: InputEvent) {
    device.write_event(&event).unwrap();
}


fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Enable) => {
            println!("Enabling mouse keys");
            let socket = UnixDatagram::unbound()?;
            socket.send_to(b"enable", "/tmp/jemclicks.sock")?;
            std::process::exit(0);
        }
        Some(Commands::Disable) => {
            println!("Disabling mouse keys");
            let socket = UnixDatagram::unbound()?;
            socket.send_to(b"disable", "/tmp/jemclicks.sock")?;
            std::process::exit(0);
        }
        None => {
            if cli.device.is_none() {
                print_devices();

                std::process::exit(0);
            }

            println!("config: {:?}", cli.config.as_deref());
            println!("device: {:?}", cli.device.as_deref());

            // let config_str = fs::read_to_string(cli.config.as_deref().unwrap_or("jemclicks.yaml"))?;

            // TODO: Make config work again
            // let config = YamlLoader::load_from_str(&config_str).unwrap_or(vec![]);

            let quit_key = EventCode::EV_KEY(EV_KEY::KEY_ESC);
            // IJKL keys are used to move the mouse by default.
            // let up_key = config[0]["UP_KEY"].as_i64().unwrap_or(23) as u16;
            // let left_key = config[0]["LEFT_KEY"].as_i64().unwrap_or(36) as u16;
            // let down_key = config[0]["DOWN_KEY"].as_i64().unwrap_or(37) as u16;
            // let right_key = config[0]["RIGHT_KEY"].as_i64().unwrap_or(38) as u16;

            // SDF keys are used to click the mouse by default.
            // let left_button = config[0]["LEFT_BUTTON"].as_i64().unwrap_or(31) as u16;
            // let middle_button = config[0]["MIDDLE_BUTTON"].as_i64().unwrap_or(32) as u16;
            // let right_button = config[0]["RIGHT_BUTTON"].as_i64().unwrap_or(33) as u16;

            // IJKL keys are used to move the mouse by default.
            let up_key = EventCode::EV_KEY(EV_KEY::KEY_I);
            let left_key = EventCode::EV_KEY(EV_KEY::KEY_J);
            let down_key = EventCode::EV_KEY(EV_KEY::KEY_K);
            let right_key = EventCode::EV_KEY(EV_KEY::KEY_L);

            // SDF keys are used to click the mouse by default.
            let left_button = EventCode::EV_KEY(EV_KEY::KEY_S);
            let middle_button = EventCode::EV_KEY(EV_KEY::KEY_D);
            let right_button = EventCode::EV_KEY(EV_KEY::KEY_F);

            let mut d = Device::new_from_path("/dev/input/event".to_string() + &cli.device.unwrap())?;

            if let Some(n) = d.name() {
                println!("Connected to device: '{}' ({:04x}:{:04x})", n, d.vendor_id(), d.product_id());
            }

            let v = init_uinput_device()?;

            // Configurables
            let freq = 90;
            let ramp_ms = 200;
            let movement_per_s = 500;

            let sleep_ms = 1000 / freq;
            let steps = (ramp_ms as f32 / sleep_ms as f32).ceil() as i32;
            let mouse_speed = movement_per_s as f32 / freq as f32;
            let speed_increment = mouse_speed / steps as f32;
            println!("steps: {}, mouse_speed_tick: {}, speed_increment: {}", steps, mouse_speed, speed_increment);

            let mut x: i32;
            let mut y: i32;

            let mut up_speed = 0.0;
            let mut down_speed = 0.0;
            let mut left_speed = 0.0;
            let mut right_speed = 0.0;

            let mut pressed_keys = HashSet::new();
            let mut grabbed = false;
            let enabled_lock = Arc::new(Mutex::new(false));

            let e_lock = Arc::clone(&enabled_lock);
            thread::spawn(move || {
                let socket_path = Path::new("/tmp/jemclicks.sock");

                // Delete old socket if necessary
                if socket_path.exists() {
                    fs::remove_file(&socket_path).unwrap();
                }

                let socket = UnixDatagram::bind(socket_path).unwrap();
                fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o777)).unwrap();

                let mut buf = vec![0; 1024];
                loop {
                    let result = socket.recv(buf.as_mut_slice()).expect("Failed to receive data");
                    let received = &buf[..result];
                    let received = String::from_utf8_lossy(received);
                    println!("Received: {:?}", received);

                    if received == "enable" {
                        let mut enabled = e_lock.lock().unwrap();
                        *enabled = true;
                    } else if received == "disable" {
                        let mut enabled = e_lock.lock().unwrap();
                        *enabled = false;
                    }
                }
            });

            let e_lock_2 = Arc::clone(&enabled_lock);
            loop {
                thread::sleep(Duration::from_millis(sleep_ms));

                let mut enabled = e_lock_2.lock().unwrap();

                if *enabled {
                    if !grabbed {
                        d.grab(GrabMode::Grab).unwrap();
                        grabbed = true;
                    }

                    let next_event = d.next_event(ReadFlag::NORMAL);
                    match next_event {
                        Ok((_, event)) => {
                            // println!("event: {:?} {:?}", event.event_code, event.value);
                            let event_type = event.event_type().unwrap_or(EventType::EV_SYN);
                            if event_type == EventType::EV_KEY {
                                if event.value == 0 {
                                    pressed_keys.remove(&event.event_code);
                                } else {
                                    pressed_keys.insert(event.event_code);
                                }
                            }
                        },
                        Err(_e) => {
                            // println!("Error: {}", _e);
                        }
                    }

                    if cli.verbose {
                        println!("[debug] pressed_keys: {:?}", pressed_keys);
                    }

                    if pressed_keys.contains(&quit_key) {
                        *enabled = false;
                        drop(enabled);
                        continue;
                    }

                    let (up, down, left, right) = detect_directions(&pressed_keys, &up_key, &down_key, &left_key, &right_key);
                    let (left_click, right_click, middle_click) = detect_mouse(&pressed_keys, &left_button, &right_button, &middle_button);

                    if up {
                        if up_speed < mouse_speed { up_speed += speed_increment; }
                    } else {
                        if up_speed > 0.0 { up_speed -= speed_increment; }
                        if up_speed < 0.0 { up_speed = 0.0; }
                    }

                    if down {
                        if down_speed < mouse_speed { down_speed += speed_increment; }
                    } else {
                        if down_speed > 0.0 { down_speed -= speed_increment; }
                        if down_speed < 0.0 { down_speed = 0.0; }
                    }

                    if left {
                        if left_speed < mouse_speed { left_speed += speed_increment; }
                    } else {
                        if left_speed > 0.0 { left_speed -= speed_increment; }
                        if left_speed < 0.0 { left_speed = 0.0; }
                    }

                    if right {
                        if right_speed < mouse_speed { right_speed += speed_increment; }
                    } else {
                        if right_speed > 0.0 { right_speed -= speed_increment; }
                        if right_speed < 0.0 { right_speed = 0.0; }
                    }

                    x = (right_speed - left_speed) as i32;
                    y = (down_speed - up_speed) as i32; // Invert y axis, because evdev is weird

                    if cli.verbose {
                        println!("[debug] x: {}, y: {}", x, y);
                    }

                    write_btn_event(&v, left_click, middle_click, right_click).unwrap();

                    if x != 0 {
                        write_x_input_event(&v, x)?;
                    }
                    if y != 0 {
                        write_y_input_event(&v, y)?;
                    }

                    write_syn(&v).unwrap();
                } else {
                    if grabbed {
                        d.grab(GrabMode::Ungrab).unwrap();
                        grabbed = false;
                    }
                }
            }
        }
    }
}
