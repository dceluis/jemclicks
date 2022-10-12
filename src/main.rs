// we import the necessary modules (only the core X module in this application).
use std::{thread, time::Duration};
use std::sync::RwLock;
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashSet;

use evdev_rs::enums::{BusType, EventCode, EventType, EV_KEY, EV_REL, EV_SYN};
use evdev_rs::{Device, DeviceWrapper, InputEvent, UInputDevice, UninitDevice, TimeVal};


fn pick_device() -> evdev::Device {
    use std::io::prelude::*;

    let mut chosen = String::new();
    let mut devices = evdev::enumerate().map(|t| t.1).collect::<Vec<_>>();
    // readdir returns them in reverse order from their eventN names for some reason
    devices.reverse();
    let mut args = std::env::args_os();
    args.next();
    if let Some(dev_file) = args.next() {
        chosen = dev_file.into_string().unwrap();
    } else {
        for (i, d) in devices.iter().enumerate() {
            println!("{}: {}", i, d.name().unwrap_or("Unnamed device"));
        }
        print!("Select the device [0-{}]: ", devices.len());
        let _ = std::io::stdout().flush();
        std::io::stdin().read_line(&mut chosen).unwrap();
    }
    println!("Using device {}", chosen);
    let n = chosen.trim().parse::<usize>().unwrap();
    devices.into_iter().nth(n).unwrap()
}

fn calc_directions(keycodes: &HashSet<u16>) -> (bool, bool, bool, bool) {
    // Keycodes are defined in /usr/include/linux/input-event-codes.h
    let mut up = false;
    let mut down = false;
    let mut left = false;
    let mut right = false;

    for keycode in keycodes {
        match keycode {
            72 => up = true,
            75 => left = true,
            80 => down = true,
            77 => right = true,
            _ => {}
        }
    }

    (up, down, left, right)
}

fn calc_mouse(keycodes: &HashSet<u16>) -> (bool, bool, bool) {
    // 71 = KEY_KP7
    // 72 = KEY_KP8
    // 73 = KEY_KP9
    // 75 = KEY_KP4
    // 76 = KEY_KP5
    // 77 = KEY_KP6
    // 79 = KEY_KP1
    // 80 = KEY_KP2
    // 81 = KEY_KP3
    // 82 = KEY_KP0

    let mut one = false;
    let mut two = false;
    let mut three = false;

    for keycode in keycodes {
        match keycode {
            79 => one = true, // KEY_KP1
            81 => two = true, // KEY_KP3
            71 => three = true, // KEY_KP7
            // 73 => four = true, // KEY_KP9
            _ => {}
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
    let mut d = pick_device();
    println!("Keyboard: {}", d);

    let enabled_lock = Arc::new(Mutex::new(false));
    let events_queue: Vec<(u16, i32)> = Vec::new();
    let queue_lock = Arc::new(RwLock::new(events_queue));

    // Spawn a thread to read the keyboard
    let ec_lock = Arc::clone(&queue_lock);
    let en_lock = Arc::clone(&enabled_lock);
    thread::spawn(move || {
        let mut grab = false;
        let mut grabbed = false;
        loop {
            for ev in d.fetch_events().unwrap() { // Blocks until an event is available
                if ev.event_type() == evdev::EventType::KEY {
                    if ev.code() == 183 && ev.value() == 1 {
                        let mut en = en_lock.lock().unwrap();
                        *en = true;
                        grab = true;
                    }
                    if ev.code() == 16 && ev.value() == 1 {
                        let mut en = en_lock.lock().unwrap();
                        *en = false;
                        grab = false;
                    }
                    // Record key press to be processed by main loop
                    let mut _es = ec_lock.write().unwrap();
                    _es.push((ev.code(), ev.value()));
                }
            }

            if grab && !grabbed {
                d.grab().unwrap();
                grabbed = true;
                println!("Grabbed");
            } else if !grab && grabbed {
                d.ungrab().unwrap();
                grabbed = false;
                println!("Ungrabbed");
            }
        }
    });


    let v = init_uinput_device()?;

    let freq = 30;
    let ramp_ms = 200; // seconds
    let sleep_ms = 1000 / freq;
    let steps = (ramp_ms as f32 / sleep_ms as f32).ceil() as i32;
    let mouse_speed = 16;
    let speed_increment = (mouse_speed as f32 / steps as f32) as i32;

    let mut pressed_keys = HashSet::new();
    let mut x = 0;
    let mut y = 0;

    let mut up_speed = 0;
    let mut down_speed = 0;
    let mut left_speed = 0;
    let mut right_speed = 0;

    let em_lock = Arc::clone(&queue_lock);
    let enm_lock = Arc::clone(&enabled_lock);
    loop {
        thread::sleep(Duration::from_millis(sleep_ms));

        if !*enm_lock.lock().unwrap() {
            continue;
        }

        let read_events = em_lock.read().unwrap();
        for event in read_events.iter() {
            let (code, value) = event;

            if value != &0 {
                pressed_keys.insert(*code);
            } else {
                pressed_keys.remove(code);
            }
        }
        drop(read_events);

        let (up, down, left, right) = calc_directions(&pressed_keys);
        let (left_click, right_click, middle_click) = calc_mouse(&pressed_keys);

        if up {
            if up_speed < mouse_speed { up_speed += speed_increment; }
        } else {
            if up_speed > 0 { up_speed -= speed_increment; }
            if up_speed < 0 { up_speed = 0; }
        }

        if down {
            if down_speed < mouse_speed { down_speed += speed_increment; }
        } else {
            if down_speed > 0 { down_speed -= speed_increment; }
            if down_speed < 0 { down_speed = 0; }
        }

        if left {
            if left_speed < mouse_speed { left_speed += speed_increment; }
        } else {
            if left_speed > 0 { left_speed -= speed_increment; }
            if left_speed < 0 { left_speed = 0; }
        }

        if right {
            if right_speed < mouse_speed { right_speed += speed_increment; }
        } else {
            if right_speed > 0 { right_speed -= speed_increment; }
            if right_speed < 0 { right_speed = 0; }
        }

        x = right_speed - left_speed;
        y = down_speed - up_speed; // Invert y axis, because evdev is weird

        println!("---");
        println!("pressed_keys: {:?}", pressed_keys);
        println!("x: {}, y: {}", x, y);
        println!("");

        write_btn_event(&v, left_click, middle_click, right_click).unwrap();

        if x != 0 {
            write_x_input_event(&v, x)?;
        }
        if y != 0 {
            write_y_input_event(&v, y)?;
        }

        write_syn(&v).unwrap();

        // Clear the events queue
        let mut write_events = em_lock.write().unwrap();
        *write_events = Vec::new();
    }
}
