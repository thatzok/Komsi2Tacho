#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use embedded_can::Frame;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Io;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::twai::{BaudRate, EspTwaiFrame, ExtendedId, TwaiConfiguration, TwaiMode};
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use Komsi2Tacho::can::{can_rx_task, can_send_frame, can_tx_task};
use Komsi2Tacho::komsi::komsi_task;
use Komsi2Tacho::time::get_current_time_for_j1939;

use embedded_io_async::Read;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // USB Serial JTAG Initialisierung
    let usb_serial = UsbSerialJtag::new(peripherals.USB_DEVICE).into_async();
    spawner.spawn(komsi_task(usb_serial)).unwrap();

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

    // let mut twai = can_config.start().into_async();
    // twai.start();
    let can_config_async = can_config.into_async();
    let mut twai = can_config_async.start();
    // Hier den Treiber in Sender und Empfänger zerteilen
    let (rx, tx) = twai.split();

    // Beide Tasks separat starten
    spawner.spawn(can_tx_task(tx)).unwrap();
    spawner.spawn(can_rx_task(rx)).unwrap();

    // spawner.spawn(can_tx_task(twai)).unwrap();

    info!("Komsi2Tacho: TWAI/CAN initialisiert (250k).");

    let mut last_send = Instant::now();
    let send_interval = Duration::from_secs(1);

    loop {
        // Sende das Demo-Paket im Sekundentakt
        if last_send.elapsed() >= send_interval {
            // 18FEE6EE#243412024029837D
            // ID: 0x18FEE6EE (Extended)
            // Daten: 24 34 12 02 40 29 83 7D
            // let id = ExtendedId::new(0x18FEE6EE).unwrap();
            // let data = [0x24, 0x34, 0x12, 0x02, 0x40, 0x29, 0x83, 0x7D];
            // let frame = EspTwaiFrame::new(id, &data).unwrap();
            // match can.transmit(&frame) {
            //    Ok(_) => info!("CAN TX: 18FEE6EE#243412024029837D"),
            //    Err(NbError::WouldBlock) => error!("CAN TX: WouldBlock"),
            //    Err(NbError::Other(e)) => error!("CAN TX Fehler: {:?}", e),
            // }

            if let Some(dt) = get_current_time_for_j1939() {
                let timedate = j1939::spn::TimeDate {
                    year: dt.year as i32,
                    month: dt.month as u32,
                    day: dt.day as u32,
                    hour: dt.hour as u32,
                    minute: dt.min as u32,
                    second: dt.sec as u32,
                    local_hour_offset: Some(0),
                    local_minute_offset: Some(0),
                };
                let j1939_id = j1939::IdBuilder::from_pgn(j1939::PGN::TimeDate)
                    .sa(0xee)
                    .build();
                let j1939_frame = j1939::FrameBuilder::new(j1939_id)
                    .copy_from_slice(&timedate.to_pdu())
                    .build();

                let id = ExtendedId::new(j1939_id.as_raw()).unwrap();
                let data = j1939_frame.pdu();
                let frame = EspTwaiFrame::new(id, &data).unwrap();
                can_send_frame(frame).await;
                info!("Zyklisches TimeDate gesendet");
                /*
                match can.transmit(&frame) {
                    Ok(_) => info!("CAN TX: DateTime Frame"),
                    Err(NbError::WouldBlock) => error!("CAN TX: WouldBlock"),
                    Err(NbError::Other(e)) => error!("CAN TX Fehler: {:?}", e),
                }
                */
            }

            last_send = Instant::now();
        }

        /*
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
         */
        Timer::after_millis(100).await; // Gibt dem Executor Zeit für andere Tasks!
    }
}
