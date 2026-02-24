use defmt::{info, Format};
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use esp_hal::Async;
use embedded_io_async::Read;

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
    Ignition(bool),         // A
    Engine(bool),           // B
    PassengerDoorsOpen(bool), // C
    Indicator(u8),          // D (0: Off, 1: Left, 2: Right)
    FixingBrake(bool),      // E
    WarningLights(bool),    // F
    MainLights(bool),       // G
    FrontDoor(bool),        // H
    SecondDoor(bool),       // I
    ThirdDoor(bool),        // J
    StopRequest(bool),      // K
    StopBrake(bool),        // L
    HighBeam(bool),         // M
    BatteryLight(bool),     // N
    SimulatorType(u8),      // O
    DoorEnable(bool),       // P
    Odometer(u64),          // o
    DateTime(KomsiDateTime), // r
    MaxSpeed(u32),          // s
    RPM(u32),               // t
    Pressure(u32),          // u
    Temperature(u32),       // v
    Oil(u32),               // w
    Fuel(u8),               // x (0-100)
    Speed(u32),             // y
    Water(u32),             // z
}

impl Command {
    pub fn from_parts(cmd_char: char, digits: &[u8]) -> Result<Self, KomsiError> {
        let value_u64 = parse_u64(digits)?;

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

fn parse_u64(digits: &[u8]) -> Result<u64, KomsiError> {
    if digits.is_empty() {
        return Err(KomsiError::InvalidValue);
    }
    let mut res: u64 = 0;
    for &d in digits {
        let digit = (d - b'0') as u64;
        res = res.saturating_mul(10).saturating_add(digit);
    }
    Ok(res)
}

fn parse_datetime(digits: &[u8]) -> Result<KomsiDateTime, KomsiError> {
    if digits.len() != 14 {
        return Err(KomsiError::InvalidDateTime);
    }

    let year = parse_slice_u16(&digits[0..4])?;
    let month = parse_slice_u8(&digits[4..6])?;
    let day = parse_slice_u8(&digits[6..8])?;
    let hour = parse_slice_u8(&digits[8..10])?;
    let min = parse_slice_u8(&digits[10..12])?;
    let sec = parse_slice_u8(&digits[12..14])?;

    Ok(KomsiDateTime {
        year,
        month,
        day,
        hour,
        min,
        sec,
    })
}

fn parse_slice_u8(slice: &[u8]) -> Result<u8, KomsiError> {
    let mut res: u8 = 0;
    for &d in slice {
        res = res.checked_mul(10).ok_or(KomsiError::InvalidValue)?
            .checked_add(d - b'0').ok_or(KomsiError::InvalidValue)?;
    }
    Ok(res)
}

fn parse_slice_u16(slice: &[u8]) -> Result<u16, KomsiError> {
    let mut res: u16 = 0;
    for &d in slice {
        res = res.checked_mul(10).ok_or(KomsiError::InvalidValue)?
            .checked_add((d - b'0') as u16).ok_or(KomsiError::InvalidValue)?;
    }
    Ok(res)
}
#[embassy_executor::task]
pub async fn komsi_task(mut usb: UsbSerialJtag<'static, Async>) {
    defmt::info!("KOMSI Task gestartet");
    let mut buffer = [0u8; 64];

    // Wir lagern den Parser-Zustand aus, um komsi_task übersichtlich zu halten
    let mut current_cmd: Option<char> = None;
    let mut digit_buffer = [0u8; 16];
    let mut digit_count = 0;

    loop {
        // Nutze .read() statt .read_async() für esp-hal 1.0.0
        match usb.read(&mut buffer).await {
            Ok(len) if len > 0 => {
                for &byte in &buffer[..len] {
                    defmt::info!("Byte empfangen: {:x} ({})", byte, byte as char); // Debug-Log
                    let c = byte as char;
                    if c.is_ascii_alphabetic() {
                        // 1. Alten Befehl abschließen (falls vorhanden)
                        if let Some(cmd) = current_cmd {
                            komsi_dispatch(cmd, &digit_buffer[..digit_count]);
                        }
                        // 2. Neuen Befehl vorbereiten
                        current_cmd = Some(c);
                        digit_count = 0;
                    }
                    else if c.is_ascii_digit() {
                        if current_cmd.is_some() {
                            if digit_count < digit_buffer.len() {
                                digit_buffer[digit_count] = byte;
                                digit_count += 1;
                            }
                        }
                    }
                    else if c == '\n' || c == '\r' || c == ';' || c == ',' {
                        // Trennzeichen schließt den aktuellen Befehl ab
                        if let Some(cmd) = current_cmd {
                            komsi_dispatch(cmd, &digit_buffer[..digit_count]);
                            current_cmd = None;
                            digit_count = 0;
                        }
                    }
                }
            }
            Ok(_) => {} // Leeres Lesen ignorieren
            Err(e) => {
                defmt::error!("USB Read Error: {:?}", e);
                // Kurze Pause, um bei Dauerfehlern die CPU nicht zu grillen
                embassy_time::Timer::after_millis(100).await;
            }
        }
    }
}


fn komsi_dispatch(cmd_char: char, digits: &[u8]) {
    match Command::from_parts(cmd_char, digits) {
        Ok(cmd) => {
            info!("KOMSI Befehl: {:?}", cmd);
            // TODO: Hier die Logik zur Ansteuerung des Tachos / TWAI einfügen
        }
        Err(e) => {
            defmt::error!("KOMSI Fehler: {:?} bei Befehl {}", e, cmd_char);
        }
    }
}
