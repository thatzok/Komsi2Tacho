use defmt::Format;
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use esp_hal::Async;
use embedded_io_async::Read;

#[derive(Debug, Format)]
pub enum KomsiError {
    UsbReadError,
    InvalidCommand(char),
    InvalidValue,
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
    DateTime(u64),          // r (YYYYMMDDHHMMSS as u64 is fine for parsing or keep as string)
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
    pub fn from_parts(cmd_char: char, value: u64) -> Result<Self, KomsiError> {
        match cmd_char {
            'A' => Ok(Command::Ignition(value != 0)),
            'B' => Ok(Command::Engine(value != 0)),
            'C' => Ok(Command::PassengerDoorsOpen(value != 0)),
            'D' => Ok(Command::Indicator(value as u8)),
            'E' => Ok(Command::FixingBrake(value != 0)),
            'F' => Ok(Command::WarningLights(value != 0)),
            'G' => Ok(Command::MainLights(value != 0)),
            'H' => Ok(Command::FrontDoor(value != 0)),
            'I' => Ok(Command::SecondDoor(value != 0)),
            'J' => Ok(Command::ThirdDoor(value != 0)),
            'K' => Ok(Command::StopRequest(value != 0)),
            'L' => Ok(Command::StopBrake(value != 0)),
            'M' => Ok(Command::HighBeam(value != 0)),
            'N' => Ok(Command::BatteryLight(value != 0)),
            'O' => Ok(Command::SimulatorType(value as u8)),
            'P' => Ok(Command::DoorEnable(value != 0)),
            'o' => Ok(Command::Odometer(value)),
            'r' => Ok(Command::DateTime(value)),
            's' => Ok(Command::MaxSpeed(value as u32)),
            't' => Ok(Command::RPM(value as u32)),
            'u' => Ok(Command::Pressure(value as u32)),
            'v' => Ok(Command::Temperature(value as u32)),
            'w' => Ok(Command::Oil(value as u32)),
            'x' => Ok(Command::Fuel(value as u8)),
            'y' => Ok(Command::Speed(value as u32)),
            'z' => Ok(Command::Water(value as u32)),
            _ => Err(KomsiError::InvalidCommand(cmd_char)),
        }
    }
}
#[embassy_executor::task]
pub async fn komsi_task(mut usb: UsbSerialJtag<'static, Async>) {
    defmt::info!("KOMSI Task gestartet");
    let mut buffer = [0u8; 64];

    // Wir lagern den Parser-Zustand aus, um komsi_task übersichtlich zu halten
    let mut current_cmd: Option<char> = None;
    let mut current_value: u64 = 0;

    loop {
        // Nutze .read() statt .read_async() für esp-hal 1.0.0
        match usb.read(&mut buffer).await {
            Ok(len) if len > 0 => {
                for &byte in &buffer[..len] {
                    let c = byte as char;

                    if c.is_ascii_alphabetic() {
                        // 1. Alten Befehl abschließen (falls vorhanden)
                        if let Some(cmd) = current_cmd {
                            komsi_dispatch(cmd, current_value);
                        }
                        // 2. Neuen Befehl vorbereiten
                        current_cmd = Some(c);
                        current_value = 0;
                    }
                    else if c.is_ascii_digit() {
                        if let Some(_) = current_cmd {
                            // Sicherer Überlaufschutz: Wert bei Max stoppen statt umspringen
                            let digit = (byte - b'0') as u64;
                            current_value = current_value
                                .saturating_mul(10)
                                .saturating_add(digit);
                        }
                    }
                    else if c.is_whitespace() || c == ';' || c == ',' {
                        // Trennzeichen schließt den aktuellen Befehl ab
                        if let Some(cmd) = current_cmd {
                            komsi_dispatch(cmd, current_value);
                            current_cmd = None;
                            current_value = 0;
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


fn komsi_dispatch(cmd_char: char, value: u64) {
    match Command::from_parts(cmd_char, value) {
        Ok(cmd) => {
            defmt::info!("KOMSI Befehl: {:?}", cmd);
            // TODO: Hier die Logik zur Ansteuerung des Tachos / TWAI einfügen
        }
        Err(e) => {
            defmt::error!("KOMSI Fehler: {:?}", e);
        }
    }
}
