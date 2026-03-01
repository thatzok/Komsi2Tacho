#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Io;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::twai::{BaudRate, TwaiConfiguration, TwaiMode};
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use komsi::KomsiDateTime;
use komsi2tacho::can::{can_manager_task, date_time_task, hr_distance_task, tachograph_task};
use komsi2tacho::commands::{komsi_task, usb_write};
use komsi2tacho::time::sync_system_time;

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

    // Set initial date and time
    sync_system_time(KomsiDateTime {
        year: 2026,
        month: 2,
        day: 13,
        hour: 8,
        min: 0,
        sec: 0,
    });

    // USB Serial JTAG initialization
    let usb_serial = UsbSerialJtag::new(peripherals.USB_DEVICE).into_async();
    spawner.spawn(komsi_task(usb_serial)).unwrap();

    // RTOS / Embassy Setup
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    // Initialization of IO pins
    let _io = Io::new(peripherals.IO_MUX);

    // In esp-hal 1.0.0 (C6) it is different, sometimes io.gpio6 or io.pins.gpio6
    // or via peripherals like here
    // It is even possible that the order of ports/pins in the parameters is not RX TX (as currently) but exactly the opposite.
    // That's the curse of an unstable API
    // as long as we stay with our current software versions, we are fine.
    let can_config = TwaiConfiguration::new(
        peripherals.TWAI0,
        peripherals.GPIO7, // TWAI_RX
        peripherals.GPIO6, // TWAI_TX
        BaudRate::B250K,
        TwaiMode::Normal, // Normal mode (send & receive), for a self-test: TwaiMode::SelfTest
    );

    // now we initialize the CAN bus
    // a bit tricky, took some time to find out the right way
    let can_config_async = can_config.into_async();
    let twai = can_config_async.start();

    // Start tasks
    spawner.spawn(can_manager_task(twai)).unwrap();
    spawner.spawn(hr_distance_task()).unwrap(); // sends distance info to Tacho and updates values
    spawner.spawn(tachograph_task()).unwrap(); // sends speed data to Tacho 
    spawner.spawn(date_time_task()).unwrap(); // sends datetime info to Tacho

    info!(
        "Komsi2Tacho Version {}: TWAI/CAN initialized (250k).",
        env!("CARGO_PKG_VERSION")
    );

    usb_write("Komsi2Tacho: CAN-Bus started (250 kbit/s).");

    // we do nothing in main loop, all work is done by tasks
    loop {
        Timer::after_millis(500).await; // Give the executor time for other tasks!
    }
}
