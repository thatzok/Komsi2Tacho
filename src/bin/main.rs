#![no_std]
#![no_main]

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use embedded_can::{Frame, Id};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Io;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::twai::{BaudRate, EspTwaiFrame, ExtendedId, TwaiConfiguration, TwaiMode};
use nb::Error as NbError;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // RTOS / Embassy Setup
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    // Initialisierung der IO-Pins
    let _io = Io::new(peripherals.IO_MUX);
    // In esp-hal 1.0.0 (C6) ist es unterschiedlich, mal io.gpio6 oder io.pins.gpio6
    // oder über peripherals wie hier
    // Es kann sogar sein, dass die Reihenfolge der Ports/Pins in den Parametern nicht RX TX (wie hier aktuell) ist sondern genau anders herum.
    // Das ist halt der Fluch einer noch unstable API
    let can_config = TwaiConfiguration::new(
        peripherals.TWAI0,
        peripherals.GPIO7, // TWAI_RX
        peripherals.GPIO6, // TWAI_TX
        BaudRate::B250K,
        TwaiMode::Normal, //  Normaler Modus (Senden & Empfangen), für einen Selbsttest:TwaiMode::SelfTest 
    );
      
    
    // Controller in den Betriebsmodus versetzen
    let mut can = can_config.start();

    info!("Komsi2Tacho: TWAI/CAN initialisiert (250k).");

    let mut last_send = Instant::now();
    let send_interval = Duration::from_secs(1);

    loop {
        // Sende das Demo-Paket im Sekundentakt
        if last_send.elapsed() >= send_interval {
            // 18FEE6EE#243412024029837D
            // ID: 0x18FEE6EE (Extended)
            // Daten: 24 34 12 02 40 29 83 7D

            let id = ExtendedId::new(0x18FEE6EE).unwrap();
            let data = [0x24, 0x34, 0x12, 0x02, 0x40, 0x29, 0x83, 0x7D];
            let frame = EspTwaiFrame::new(id, &data).unwrap();

            match can.transmit(&frame) {
                Ok(_) => info!("CAN TX: 18FEE6EE#243412024029837D"),
                Err(NbError::WouldBlock) => error!("CAN TX: WouldBlock"),
                Err(NbError::Other(e)) => error!("CAN TX Fehler: {:?}", e),
            }

            last_send = Instant::now();
        }

        match can.receive() {
            Ok(frame) => {
                // ID extrahieren (Standard oder Extended)
                let id: u32 = match frame.id() {
                    Id::Standard(id) => id.as_raw() as u32,
                    Id::Extended(id) => id.as_raw(),
                };

                let data = frame.data();
                let dlc = frame.dlc() as usize;

                info!("CAN RX: ID={:08X} DLC={} Data={}", id, dlc, &data[..dlc]);
            }
            Err(NbError::WouldBlock) => {
                // Kein Frame verfügbar -> normal bei non-blocking APIs
                // Optional: kurz warten, damit es nicht busy-loopt
                Timer::after(Duration::from_millis(10)).await;
            }
            Err(NbError::Other(e)) => {
                // Hier steckt ein echter TWAI-Fehler drin.
                error!("CAN Bus Fehler (TWAI): {:?}", e);
                Timer::after(Duration::from_millis(500)).await;
            }
        }
    }
}
