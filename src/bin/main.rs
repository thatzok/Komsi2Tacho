#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Io, Level, Output, OutputConfig, Pull};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::twai::{BaudRate, TwaiConfiguration, TwaiMode};
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use komsi::KomsiDateTime;
use komsi2tacho::can::{
    can_manager_task, can_self_test_task, date_time_task, hr_distance_task, tachograph_task,
};
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

    // wait time for USB-Serial-JTAG Zeit to connect with PC so the first messages are not lost in buffer
    embassy_time::Timer::after(Duration::from_millis(2000)).await;
    for _ in 0..5 {
        usb_write("");
    }
    usb_write("========================================");

    // Check for Debug Mode (GPIO 10)
    let debug_pin = Input::new(
        peripherals.GPIO10,
        InputConfig::default().with_pull(Pull::Up),
    );
    let mut is_debug_mode = debug_pin.is_low();
    // is_debug_mode = true; // temporary debug mode override for tests in development

    let twai_mode = if is_debug_mode {
        TwaiMode::SelfTest
    } else {
        TwaiMode::Normal
    };

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
        twai_mode,
    );

    // now we initialize the CAN bus
    // a bit tricky, took some time to find out the right way
    let can_config_async = can_config.into_async();
    let twai = can_config_async.start();

    // Start tasks
    if is_debug_mode {
        info!("Debug Mode enabled, CAN-Mode: Self-Test");
        spawner.spawn(can_self_test_task(twai)).unwrap();
    } else {
        info!("No Debug Mode enabled, CAN Mode: Normal operation");
        spawner.spawn(can_manager_task(twai)).unwrap();
        spawner.spawn(hr_distance_task()).unwrap(); // sends distance info to Tacho and updates values
        spawner.spawn(tachograph_task()).unwrap(); // sends speed data to Tacho
        spawner.spawn(date_time_task()).unwrap(); // sends datetime info to Tacho
    }

    info!(
        "Komsi2Tacho Version {}: TWAI/CAN initialized (250k, Mode: {:?}).",
        env!("CARGO_PKG_VERSION"),
        twai_mode
    );

    usb_write(concat!(
        "Komsi2Tacho Version ",
        env!("CARGO_PKG_VERSION"),
        " started"
    ));

    // we do nothing in main loop, all work is done by tasks
    loop {
        Timer::after_millis(500).await; // Give the executor time for other tasks!
    }
}
