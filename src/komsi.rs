use crate::time::sync_system_time;
use defmt::{Format, debug, error, info, warn};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embedded_io_async::{Read, Write};
use esp_hal::Async;
use esp_hal::usb_serial_jtag::UsbSerialJtag;

// "Global" Variables thread safe for vehicle state
pub static ACTUAL_SPEED: Mutex<CriticalSectionRawMutex, core::cell::Cell<u32>> =
    Mutex::new(core::cell::Cell::new(0));
pub static MAX_SPEED: Mutex<CriticalSectionRawMutex, core::cell::Cell<u32>> =
    Mutex::new(core::cell::Cell::new(0));
pub static TOTAL_DISTANCE: Mutex<CriticalSectionRawMutex, core::cell::Cell<u64>> =
    Mutex::new(core::cell::Cell::new(0));
pub static TRIP_DISTANCE: Mutex<CriticalSectionRawMutex, core::cell::Cell<u64>> =
    Mutex::new(core::cell::Cell::new(0));

#[derive(Debug, Format)]
pub enum KomsiError {
    UsbReadError,
    InvalidCommand(char),
    InvalidValue,
    InvalidDateTime,
}

#[derive(Debug, Format, Clone, Copy)]
pub struct KomsiDateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
}

#[derive(Debug, Format)]
pub enum Command {
    Ignition(bool),           // A
    Engine(bool),             // B
    PassengerDoorsOpen(bool), // C
    Indicator(u8),            // D
    FixingBrake(bool),        // E
    WarningLights(bool),      // F
    MainLights(bool),         // G
    FrontDoor(bool),          // H
    SecondDoor(bool),         // I
    ThirdDoor(bool),          // J
    StopRequest(bool),        // K
    StopBrake(bool),          // L
    HighBeam(bool),           // M
    BatteryLight(bool),       // N
    SimulatorType(u8),        // O
    DoorEnable(bool),         // P
    Odometer(u64),            // o
    DateTime(KomsiDateTime),  // r
    MaxSpeed(u32),            // s
    RPM(u32),                 // t
    Pressure(u32),            // u
    Temperature(u32),         // v
    Oil(u32),                 // w
    Fuel(u8),                 // x
    Speed(u32),               // y
    Water(u32),               // z
}

impl Command {
    pub fn from_parts(cmd_char: char, digits: &[u8]) -> Result<Self, KomsiError> {
        // default value 0 if we do not received a value
        let value_u64 = if digits.is_empty() {
            0
        } else {
            parse_u64(digits)?
        };

        match cmd_char {
            'A' => Ok(Command::Ignition(value_u64 != 0)),
            'B' => Ok(Command::Engine(value_u64 != 0)),
            'C' => Ok(Command::PassengerDoorsOpen(value_u64 != 0)),
            'D' => Ok(Command::Indicator(value_u64 as u8)),
            'E' => Ok(Command::FixingBrake(value_u64 != 0)),
            'F' => Ok(Command::WarningLights(value_u64 != 0)),
            'G' => Ok(Command::MainLights(value_u64 != 0)),
            'H' => Ok(Command::FrontDoor(value_u64 != 0)),
            'I' => Ok(Command::SecondDoor(value_u64 != 0)),
            'J' => Ok(Command::ThirdDoor(value_u64 != 0)),
            'K' => Ok(Command::StopRequest(value_u64 != 0)),
            'L' => Ok(Command::StopBrake(value_u64 != 0)),
            'M' => Ok(Command::HighBeam(value_u64 != 0)),
            'N' => Ok(Command::BatteryLight(value_u64 != 0)),
            'O' => Ok(Command::SimulatorType(value_u64 as u8)),
            'P' => Ok(Command::DoorEnable(value_u64 != 0)),
            'o' => Ok(Command::Odometer(value_u64)),
            'r' => Ok(Command::DateTime(parse_datetime(digits)?)),
            's' => Ok(Command::MaxSpeed(value_u64 as u32)),
            't' => Ok(Command::RPM(value_u64 as u32)),
            'u' => Ok(Command::Pressure(value_u64 as u32)),
            'v' => Ok(Command::Temperature(value_u64 as u32)),
            'w' => Ok(Command::Oil(value_u64 as u32)),
            'x' => Ok(Command::Fuel(value_u64 as u8)),
            'y' => Ok(Command::Speed(value_u64 as u32)),
            'z' => Ok(Command::Water(value_u64 as u32)),
            _ => Err(KomsiError::InvalidCommand(cmd_char)),
        }
    }
}

// Helperfunctions for Parsing

fn parse_u64(digits: &[u8]) -> Result<u64, KomsiError> {
    let mut res: u64 = 0;
    for &d in digits {
        let digit = d.checked_sub(b'0').ok_or(KomsiError::InvalidValue)? as u64;
        if digit > 9 {
            return Err(KomsiError::InvalidValue);
        }
        res = res.saturating_mul(10).saturating_add(digit);
    }
    Ok(res)
}

fn parse_datetime(digits: &[u8]) -> Result<KomsiDateTime, KomsiError> {
    if digits.len() != 14 {
        return Err(KomsiError::InvalidDateTime);
    }
    Ok(KomsiDateTime {
        year: parse_slice_u16(&digits[0..4])?,
        month: parse_slice_u8(&digits[4..6])?,
        day: parse_slice_u8(&digits[6..8])?,
        hour: parse_slice_u8(&digits[8..10])?,
        min: parse_slice_u8(&digits[10..12])?,
        sec: parse_slice_u8(&digits[12..14])?,
    })
}

fn parse_slice_u8(slice: &[u8]) -> Result<u8, KomsiError> {
    let mut res: u8 = 0;
    for &d in slice {
        let digit = d.checked_sub(b'0').ok_or(KomsiError::InvalidValue)?;
        res = res
            .checked_mul(10)
            .ok_or(KomsiError::InvalidValue)?
            .checked_add(digit)
            .ok_or(KomsiError::InvalidValue)?;
    }
    Ok(res)
}

fn parse_slice_u16(slice: &[u8]) -> Result<u16, KomsiError> {
    let mut res: u16 = 0;
    for &d in slice {
        let digit = d.checked_sub(b'0').ok_or(KomsiError::InvalidValue)? as u16;
        res = res
            .checked_mul(10)
            .ok_or(KomsiError::InvalidValue)?
            .checked_add(digit)
            .ok_or(KomsiError::InvalidValue)?;
    }
    Ok(res)
}

#[embassy_executor::task]
pub async fn komsi_task(mut usb: UsbSerialJtag<'static, Async>) {
    info!("KOMSI Task started");

    // Send welcome message
    let _ = usb
        .write_all(b"\r\n--- KOMSI Interface Ready ---\r\n")
        .await;

    let mut buffer = [0u8; 64];
    let mut current_cmd: Option<char> = None;
    let mut digit_buffer = [0u8; 16];
    let mut digit_count = 0;

    loop {
        match usb.read(&mut buffer).await {
            Ok(len) if len > 0 => {
                for &byte in &buffer[..len] {
                    // Echo for terminal feedback
                    let _ = usb.write_all(&[byte]).await;

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
}

fn komsi_dispatch(cmd_char: char, digits: &[u8]) {
    match Command::from_parts(cmd_char, digits) {
        Ok(cmd) => {
            info!("KOMSI command detected: {:?}", cmd);

            // Process the detected command here
            match cmd {
                Command::DateTime(dt) => {
                    // Synchronize system time with the received date
                    sync_system_time(dt);
                }

                Command::Speed(speed) => {
                    // we make sure the tacho never shows more than 125 km/h because we do not want to damage the needle
                    let safe_speed = if speed > 125 { 125 } else { speed };
                    ACTUAL_SPEED.lock(|s| s.set(safe_speed));
                }

                Command::MaxSpeed(speed) => {
                    MAX_SPEED.lock(|s| s.set(speed));
                }

                Command::Odometer(dist) => {
                    TOTAL_DISTANCE.lock(|d| d.set(dist));
                    TRIP_DISTANCE.lock(|d| d.set(0));
                }

                // We could add more commands here, if we want
                // for now, just some input message as example
                Command::Ignition(on) => {
                    info!("Ignition set to {}", on);
                }

                _ => {
                    // All other commands that do not have specific logic yet
                    // silently do nothing
                }
            }
        }
        Err(e) => {
            error!("KOMSI error: {:?} at '{}'", e, cmd_char);
        }
    }
}
