use crate::time::sync_system_time;
use defmt::{error, info};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embedded_io_async::{Read, Write};
use esp_hal::Async;
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use heapless::String;
use komsi::KomsiCommand;

#[derive(Debug)]
pub enum UsbMsg {
    Static(&'static str),
    Dynamic(String<64>),
}

impl defmt::Format for UsbMsg {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            UsbMsg::Static(s) => defmt::write!(fmt, "Static({:?})", s),
            UsbMsg::Dynamic(s) => defmt::write!(fmt, "Dynamic({:?})", s.as_str()),
        }
    }
}

// Channel for USB outgoing messages
pub static USB_TX_CHANNEL: Channel<CriticalSectionRawMutex, UsbMsg, 8> = Channel::new();

/// Helper to send a message to the USB output channel
pub fn usb_write(msg: &'static str) {
    let _ = USB_TX_CHANNEL.try_send(UsbMsg::Static(msg));
}

/// Helper to send a dynamic message to the USB output channel
pub fn usb_write_dynamic(msg: String<64>) {
    let _ = USB_TX_CHANNEL.try_send(UsbMsg::Dynamic(msg));
}

// "Global" Variables thread safe for vehicle state
pub static ACTUAL_SPEED: Mutex<CriticalSectionRawMutex, core::cell::Cell<u32>> =
    Mutex::new(core::cell::Cell::new(0));
pub static MAX_SPEED: Mutex<CriticalSectionRawMutex, core::cell::Cell<u32>> =
    Mutex::new(core::cell::Cell::new(0));
pub static TOTAL_DISTANCE: Mutex<CriticalSectionRawMutex, core::cell::Cell<u64>> =
    Mutex::new(core::cell::Cell::new(0));
pub static TRIP_DISTANCE: Mutex<CriticalSectionRawMutex, core::cell::Cell<u64>> =
    Mutex::new(core::cell::Cell::new(0));

#[embassy_executor::task]
pub async fn komsi_task(mut usb: UsbSerialJtag<'static, Async>) {
    info!("KOMSI Task started");

    // Send welcome message
    let _ = usb
        .write_all(
            concat!(
                "\r\n--- Komsi2Tacho Version (v",
                env!("CARGO_PKG_VERSION"),
                ") ---\r\n--- KOMSI Interface Ready ---\r\n"
            )
            .as_bytes(),
        )
        .await;

    let mut buffer = [0u8; 64];
    let mut current_cmd: Option<char> = None;
    let mut digit_buffer = [0u8; 16];
    let mut digit_count = 0;

    loop {
        use embassy_futures::select::{Either, select};

        match select(usb.read(&mut buffer), USB_TX_CHANNEL.receive()).await {
            Either::First(read_result) => {
                match read_result {
                    Ok(len) if len > 0 => {
                        for &byte in &buffer[..len] {
                            // Echo for terminal feedback
                            // no echo let _ = usb.write_all(&[byte]).await;

                            let c = byte as char;
                            if c.is_ascii_alphabetic() {
                                if let Some(cmd) = current_cmd {
                                    komsi_dispatch(cmd, &digit_buffer[..digit_count]);
                                }
                                current_cmd = Some(c);
                                digit_count = 0;
                            } else if c.is_ascii_digit() {
                                if current_cmd.is_some() && digit_count < digit_buffer.len() {
                                    digit_buffer[digit_count] = byte;
                                    digit_count += 1;
                                }
                            } else if c == '\n' || c == '\r' || c == ';' || c == ' ' {
                                if let Some(cmd) = current_cmd {
                                    komsi_dispatch(cmd, &digit_buffer[..digit_count]);
                                    current_cmd = None;
                                    digit_count = 0;
                                }
                            }
                        }
                        let _ = usb.flush().await;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("USB Read Error: {:?}", e);
                        embassy_time::Timer::after_millis(100).await;
                    }
                }
            }
            Either::Second(msg) => {
                match msg {
                    UsbMsg::Static(s) => {
                        let _ = usb.write_all(s.as_bytes()).await;
                    }
                    UsbMsg::Dynamic(s) => {
                        let _ = usb.write_all(s.as_bytes()).await;
                    }
                }
                let _ = usb.write_all(b"\r\n").await;
                let _ = usb.flush().await;
            }
        }
    }
}

fn komsi_dispatch(cmd_char: char, digits: &[u8]) {
    match KomsiCommand::from_parts(cmd_char, digits) {
        Ok(cmd) => {
            info!("KOMSI command detected: {:?}", cmd);

            // Process the detected command here
            match cmd {
                KomsiCommand::DateTime(dt) => {
                    // Synchronize system time with the received date
                    sync_system_time(dt);
                    info!("OK: DateTime synchronized");
                }

                KomsiCommand::Speed(speed) => {
                    // we make sure the tacho never shows more than 125 km/h because we do not want to damage the needle
                    let safe_speed = if speed > 125 { 125 } else { speed };
                    ACTUAL_SPEED.lock(|s| s.set(safe_speed));
                    info!("OK: Speed set");
                }

                KomsiCommand::MaxSpeed(speed) => {
                    MAX_SPEED.lock(|s| s.set(speed));
                    info!("OK: MaxSpeed set");
                }

                KomsiCommand::Odometer(dist) => {
                    TOTAL_DISTANCE.lock(|d| d.set(dist));
                    TRIP_DISTANCE.lock(|d| d.set(0));
                    info!("OK: Odometer set");
                }

                // We could add more commands here, if we want
                // for now, just some input message as example
                KomsiCommand::Ignition(on) => {
                    if on {
                        info!("OK: Ignition ON");
                    } else {
                        info!("OK: Ignition OFF");
                    };
                }

                _ => {
                    // All other commands that do not have specific logic yet
                    // usb_write("OK: Command received");
                }
            }
        }
        Err(e) => {
            info!("ERR: KOMSI command: {:?}", e);
        }
    }
}
