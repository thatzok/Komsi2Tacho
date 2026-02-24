use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use defmt::{error, info, unwrap};
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use embedded_can::{Frame, Id};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Io;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::twai::{BaudRate, EspTwaiFrame, ExtendedId, TwaiConfiguration, TwaiMode};
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use esp_hal::Async;
use crate::komsi::komsi_task;
use crate::time::{get_current_time_for_j1939, sync_system_time};

// Kanal für 16 Frames - Puffer für "einmal alles senden"
pub static CAN_TX_CHANNEL: Channel<CriticalSectionRawMutex, EspTwaiFrame, 16> = Channel::new();

/// Hilfsfunktion, um von überall ein Paket in die Warteschlange zu legen
pub async fn can_send_frame(frame: EspTwaiFrame) {
    CAN_TX_CHANNEL.send(frame).await;
}

/// Der zentrale Sender-Task, der exklusiv auf die Hardware zugreift
#[embassy_executor::task]
pub async fn can_sender_task(mut twai: Twai<'static, Async>) {
    info!("CAN Sender-Task gestartet");
    loop {
        // Warte auf das nächste Paket aus dem Kanal
        let frame = CAN_TX_CHANNEL.receive().await;

        // Physisches Senden (wartet asynchron, falls Hardware-Puffer voll)
        if let Err(e) = twai.transmit(&frame).await {
            error!("Fehler beim TWAI Transmit: {:?}", e);
        }
    }
}
