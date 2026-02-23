#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::gpio::Io;
use esp_hal::rmt::Rmt;
use esp_hal_smartled::{smart_led_buffer, SmartLedsAdapter};
use smart_leds::{brightness, colors, SmartLedsWrite};



#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.2.0

    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // RMT und GPIO8 initialisieren
    let _io = Io::new(peripherals.IO_MUX);
    let rmt = Rmt::new(peripherals.RMT, esp_hal::time::Rate::from_mhz(80)).unwrap();

    // Adapter für WS2812 an GPIO8 erstellen
    let mut rmt_buffer = smart_led_buffer!(1); // Buffer für 1 LED
    let mut led = SmartLedsAdapter::new(rmt.channel0, peripherals.GPIO8, &mut rmt_buffer);

    let mut color = colors::RED;

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Embassy initialized!");

    // TODO: Spawn some tasks
    let _ = spawner;

    loop {
        info!("Hello world!");

        led.write(brightness([color].iter().cloned(), 50)).unwrap();

        // Farbe wechseln
        color = if color == colors::RED {
            colors::GREEN
        } else if color == colors::GREEN {
            colors::BLUE
        } else {
            colors::RED
        };

        Timer::after(Duration::from_millis(500)).await;
    }

}
