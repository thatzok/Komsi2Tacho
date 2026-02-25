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
use Komsi2Tacho::can::{
    can_rx_task, can_send_frame, can_tx_task, date_time_task, hr_distance_task, tachograph_task,
};
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

    // Tasks starten
    spawner.spawn(can_tx_task(tx)).unwrap();
    spawner.spawn(can_rx_task(rx)).unwrap();
    spawner.spawn(hr_distance_task()).unwrap();
    spawner.spawn(tachograph_task()).unwrap();
    spawner.spawn(date_time_task()).unwrap();

    // spawner.spawn(can_tx_task(twai)).unwrap();

    info!("Komsi2Tacho: TWAI/CAN initialisiert (250k).");

    loop {
        Timer::after_millis(100).await; // Gibt dem Executor Zeit für andere Tasks!
    }
    
}
