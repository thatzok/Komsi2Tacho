use defmt::Format;

#[derive(Debug, Format, Clone, Copy, PartialEq, Eq)]
pub enum KomsiError {
    InvalidIndicatorValue,
    InvalidSimulatorTypeValue,
    ValueTooLong,
    InvalidValue,
    IncompleteDateTime,
}

#[derive(Debug, Format, Clone, Copy, PartialEq, Eq)]
pub enum Indicator {
    Off = 0,
    Left = 1,
    Right = 2,
}

impl TryFrom<u32> for Indicator {
    type Error = KomsiError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Indicator::Off),
            1 => Ok(Indicator::Left),
            2 => Ok(Indicator::Right),
            _ => Err(KomsiError::InvalidIndicatorValue),
        }
    }
}

#[derive(Debug, Format, Clone, Copy, PartialEq, Eq)]
pub enum SimulatorType {
    Omsi2 = 0,
    TheBus = 1,
}

impl TryFrom<u32> for SimulatorType {
    type Error = KomsiError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SimulatorType::Omsi2),
            1 => Ok(SimulatorType::TheBus),
            _ => Err(KomsiError::InvalidSimulatorTypeValue),
        }
    }
}

#[derive(Debug, Format, Clone, Copy)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

#[derive(Debug, Format, Clone, Copy)]
pub enum KomsiCommand {
    Ignition(bool),
    Engine(bool),
    PassengerDoorsOpen(bool),
    Indicator(Indicator),
    FixingBrake(bool),
    WarningLights(bool),
    MainLights(bool),
    FrontDoor(bool),
    SecondDoor(bool),
    ThirdDoor(bool),
    StopRequest(bool),
    StopBrake(bool),
    HighBeam(bool),
    BatteryLight(bool),
    SimulatorType(SimulatorType),
    DoorEnable(bool),
    Odometer(u64),
    DateTime(DateTime),
    MaxSpeed(u32),
    RPM(u32),
    Pressure(u32),
    Temperature(u32),
    Oil(u32),
    Fuel(u32),
    Speed(u32),
    Water(u32),
}

pub struct KomsiParser {
    command_code: Option<char>,
    buffer: [u8; 32],
    buffer_len: usize,
}

impl KomsiParser {
    pub const fn new() -> Self {
        Self {
            command_code: None,
            buffer: [0; 32],
            buffer_len: 0,
        }
    }

    pub fn parse_char(&mut self, c: char) -> Option<KomsiCommand> {
        if c == '\n' {
            let cmd = self.finalize_current_command();
            self.reset();
            return cmd;
        }

        if c == '\r' {
            return None;
        }

        if c.is_ascii_alphabetic() {
            let previous_cmd = self.finalize_current_command();
            self.reset();
            self.command_code = Some(c);
            return previous_cmd;
        }

        if c.is_ascii_digit() {
            if self.command_code.is_some() {
                if self.buffer_len < self.buffer.len() {
                    self.buffer[self.buffer_len] = c as u8;
                    self.buffer_len += 1;
                }
            }
            return None;
        }

        // Unbekannte Zeichen triggern finalize (laut Spezifikation "next Non-Digit")
        let cmd = self.finalize_current_command();
        self.reset();
        cmd
    }

    fn reset(&mut self) {
        self.command_code = None;
        self.buffer_len = 0;
    }

    fn finalize_current_command(&mut self) -> Option<KomsiCommand> {
        let code = self.command_code?;
        if self.buffer_len == 0 {
            return None;
        }

        let val_str = core::str::from_utf8(&self.buffer[..self.buffer_len]).ok()?;

        match code {
            'A' => Some(KomsiCommand::Ignition(val_str == "1")),
            'B' => Some(KomsiCommand::Engine(val_str == "1")),
            'C' => Some(KomsiCommand::PassengerDoorsOpen(val_str == "1")),
            'D' => val_str.parse::<u32>().ok().and_then(|v| Indicator::try_from(v).ok()).map(KomsiCommand::Indicator),
            'E' => Some(KomsiCommand::FixingBrake(val_str == "1")),
            'F' => Some(KomsiCommand::WarningLights(val_str == "1")),
            'G' => Some(KomsiCommand::MainLights(val_str == "1")),
            'H' => Some(KomsiCommand::FrontDoor(val_str == "1")),
            'I' => Some(KomsiCommand::SecondDoor(val_str == "1")),
            'J' => Some(KomsiCommand::ThirdDoor(val_str == "1")),
            'K' => Some(KomsiCommand::StopRequest(val_str == "1")),
            'L' => Some(KomsiCommand::StopBrake(val_str == "1")),
            'M' => Some(KomsiCommand::HighBeam(val_str == "1")),
            'N' => Some(KomsiCommand::BatteryLight(val_str == "1")),
            'O' => val_str.parse::<u32>().ok().and_then(|v| SimulatorType::try_from(v).ok()).map(KomsiCommand::SimulatorType),
            'P' => Some(KomsiCommand::DoorEnable(val_str == "1")),
            'o' => val_str.parse::<u64>().ok().map(KomsiCommand::Odometer),
            'r' => {
                if val_str.len() == 14 {
                    let year = val_str[0..4].parse::<u16>().ok()?;
                    let month = val_str[4..6].parse::<u8>().ok()?;
                    let day = val_str[6..8].parse::<u8>().ok()?;
                    let hour = val_str[8..10].parse::<u8>().ok()?;
                    let minute = val_str[10..12].parse::<u8>().ok()?;
                    let second = val_str[12..14].parse::<u8>().ok()?;
                    Some(KomsiCommand::DateTime(DateTime { year, month, day, hour, minute, second }))
                } else {
                    None
                }
            },
            's' => val_str.parse::<u32>().ok().map(KomsiCommand::MaxSpeed),
            't' => val_str.parse::<u32>().ok().map(KomsiCommand::RPM),
            'u' => val_str.parse::<u32>().ok().map(KomsiCommand::Pressure),
            'v' => val_str.parse::<u32>().ok().map(KomsiCommand::Temperature),
            'w' => val_str.parse::<u32>().ok().map(KomsiCommand::Oil),
            'x' => val_str.parse::<u32>().ok().map(KomsiCommand::Fuel),
            'y' => val_str.parse::<u32>().ok().map(KomsiCommand::Speed),
            'z' => val_str.parse::<u32>().ok().map(KomsiCommand::Water),
            _ => None,
        }
    }
}

pub fn komsi_dispatch(command: KomsiCommand) {
    match command {
        KomsiCommand::Ignition(on) => defmt::info!("Ignition: {}", on),
        KomsiCommand::Engine(on) => defmt::info!("Engine: {}", on),
        KomsiCommand::PassengerDoorsOpen(open) => defmt::info!("PassengerDoorsOpen: {}", open),
        KomsiCommand::Indicator(ind) => defmt::info!("Indicator: {:?}", ind),
        KomsiCommand::FixingBrake(on) => defmt::info!("FixingBrake: {}", on),
        KomsiCommand::WarningLights(on) => defmt::info!("WarningLights: {}", on),
        KomsiCommand::MainLights(on) => defmt::info!("MainLights: {}", on),
        KomsiCommand::FrontDoor(open) => defmt::info!("FrontDoor: {}", open),
        KomsiCommand::SecondDoor(open) => defmt::info!("SecondDoor: {}", open),
        KomsiCommand::ThirdDoor(open) => defmt::info!("ThirdDoor: {}", open),
        KomsiCommand::StopRequest(on) => defmt::info!("StopRequest: {}", on),
        KomsiCommand::StopBrake(on) => defmt::info!("StopBrake: {}", on),
        KomsiCommand::HighBeam(on) => defmt::info!("HighBeam: {}", on),
        KomsiCommand::BatteryLight(on) => defmt::info!("BatteryLight: {}", on),
        KomsiCommand::SimulatorType(sim) => defmt::info!("SimulatorType: {:?}", sim),
        KomsiCommand::DoorEnable(on) => defmt::info!("DoorEnable: {}", on),
        KomsiCommand::Odometer(val) => defmt::info!("Odometer: {} m", val),
        KomsiCommand::DateTime(dt) => defmt::info!("DateTime: {}-{:02}-{:02} {:02}:{:02}:{:02}", dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second),
        KomsiCommand::MaxSpeed(val) => defmt::info!("MaxSpeed: {} km/h", val),
        KomsiCommand::RPM(val) => defmt::info!("RPM: {}", val),
        KomsiCommand::Pressure(val) => defmt::info!("Pressure: {}", val),
        KomsiCommand::Temperature(val) => defmt::info!("Temperature: {}", val),
        KomsiCommand::Oil(val) => defmt::info!("Oil: {}", val),
        KomsiCommand::Fuel(val) => defmt::info!("Fuel: {}%", val),
        KomsiCommand::Speed(val) => defmt::info!("Speed: {} km/h", val),
        KomsiCommand::Water(val) => defmt::info!("Water: {}", val),
    }
}
