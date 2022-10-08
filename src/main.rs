// we import the necessary modules (only the core X module in this application).
use std::{thread, time::Duration};
use std::io::{self, Write};
use std::sync::RwLock;
use std::sync::Arc;
use std::collections::HashSet;
use std::fs::File;

use evdev_rs::enums::{BusType, EventCode, EventType, EV_KEY, EV_REL, EV_SYN};
use evdev_rs::{Device, DeviceWrapper, InputEvent, ReadFlag, UInputDevice, UninitDevice, TimeVal};


fn pick_device() -> evdev::Device {
    use std::io::prelude::*;

    let mut args = std::env::args_os();
    args.next();
    if let Some(dev_file) = args.next() {
        evdev::Device::open(dev_file).unwrap()
    } else {
        let mut devices = evdev::enumerate().map(|t| t.1).collect::<Vec<_>>();
        // readdir returns them in reverse order from their eventN names for some reason
        devices.reverse();
        for (i, d) in devices.iter().enumerate() {
            println!("{}: {}", i, d.name().unwrap_or("Unnamed device"));
        }
        print!("Select the device [0-{}]: ", devices.len());
        let _ = std::io::stdout().flush();
        let mut chosen = String::new();
        std::io::stdin().read_line(&mut chosen).unwrap();
        let n = chosen.trim().parse::<usize>().unwrap();
        devices.into_iter().nth(n).unwrap()
    }
}

fn calc_direction(keycodes: &HashSet<u16>) -> (i32, i32) {
    let small_x = 10;
    let big_x = 20;
    let small_y = 8;
    let big_y = 16;
    let mut x = 0;
    let mut y = 0;

    for keycode in keycodes {
        match keycode {
            17 => x -= big_x,
            31 => x -= big_x,
            18 => y -= big_y,
            33 => x += big_x,
            45 => y += big_y,
            46 => y += big_y,

            36 => x -= small_x,
            23 => y -= small_y,
            38 => x += small_x,
            50 => y += small_y,
            _ => {}
        }
    }
    (x, y)
}
fn calc_mouse(keycodes: &HashSet<u16>) -> (bool, bool, bool) {
    let mut one = false;
    let mut two = false;
    let mut three = false;

    for keycode in keycodes {
        match keycode {
            39 => one = true,
            26 => two = true,
            16 => three = true,
            _ => {}
        }
    }
    (one, two, three)
}

fn main() -> std::io::Result<()> {
    let mut d = pick_device();
    println!("Keyboard: {}", d);

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
    let v = UInputDevice::create_from_device(&u)?;

    let mut pressed_keys = HashSet::new();

    let events: Vec<(u16, i32)> = Vec::new();
    let e_lock = Arc::new(RwLock::new(events));
    let ec_lock = Arc::clone(&e_lock);

    thread::spawn(move || {
        loop {
            for ev in d.fetch_events().unwrap() {
                // println!("{:?}", ev);
                if ev.event_type() == evdev::EventType::KEY {
                    let mut unwrapped_events = ec_lock.write().unwrap();
                    unwrapped_events.push((ev.code(), ev.value()));
                }
            }
        }
    });

    let em_lock = Arc::clone(&e_lock);

    loop {
        thread::sleep(Duration::from_millis(33));

        let read_events = em_lock.read().unwrap();

        // println!("");
        // println!("---");
        // println!("read_events: {:?}", read_events);
        for event in read_events.iter() {
            let code = event.0;
            let value = event.1;

            if value != 0 {
                pressed_keys.insert(code);
            } else {
                pressed_keys.remove(&code);
            }
        }
        drop(read_events);

        let direction = calc_direction(&pressed_keys);
        let mouse_btns = calc_mouse(&pressed_keys);
        println!("---");
        println!("pressed_keys: {:?}", pressed_keys);
        println!("direction: {:?}", direction);
        println!("---");
        println!("");
        let since_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        let seconds = since_epoch.as_secs();
        let micros = since_epoch.subsec_micros();

        if mouse_btns.0 {
            v.write_event(&InputEvent {
                time: TimeVal {
                    tv_sec: seconds as i64,
                    tv_usec: micros as i64,
                },
                event_code: EventCode::EV_KEY(EV_KEY::BTN_LEFT),
                value: 1,
            })?;
        } else {
            v.write_event(&InputEvent {
                time: TimeVal {
                    tv_sec: seconds as i64,
                    tv_usec: micros as i64,
                },
                event_code: EventCode::EV_KEY(EV_KEY::BTN_LEFT),
                value: 0,
            })?;
        }

        v.write_event(&InputEvent {
            time: TimeVal {
                tv_sec: seconds as i64,
                tv_usec: micros as i64,
            },
            event_code: EventCode::EV_REL(EV_REL::REL_X),
            value: direction.0,
        })?;

        v.write_event(&InputEvent {
            time: TimeVal {
                tv_sec: seconds as i64,
                tv_usec: micros as i64,
            },
            event_code: EventCode::EV_REL(EV_REL::REL_Y),
            value: direction.1,
        })?;

        v.write_event(&InputEvent {
            time: TimeVal {
                tv_sec: seconds as i64,
                tv_usec: micros as i64,
            },
            event_code: EventCode::EV_SYN(EV_SYN::SYN_REPORT),
            value: 0,
        })?;

        let mut write_events = em_lock.write().unwrap();
        *write_events = Vec::new();
    }
}
