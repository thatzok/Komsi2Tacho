use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use defmt::{debug, error, info, unwrap};
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
use esp_hal::twai::{TwaiTx, TwaiRx};

// Kanal für 16 Frames - Puffer für "einmal alles senden"
pub static CAN_TX_CHANNEL: Channel<CriticalSectionRawMutex, EspTwaiFrame, 16> = Channel::new();

/// Hilfsfunktion, um von überall ein Paket in die Warteschlange zu legen
pub async fn can_send_frame(frame: EspTwaiFrame) {
    CAN_TX_CHANNEL.send(frame).await;
}


/// Task für das Senden (verarbeitet die Warteschlange)
#[embassy_executor::task]
pub async fn can_tx_task(mut tx: TwaiTx<'static, Async>) {
    info!("CAN TX Task gestartet");
    loop {
        let frame = CAN_TX_CHANNEL.receive().await;
        if let Err(e) = tx.transmit_async(&frame).await {
            error!("CAN TX Error: {:?}", e);
        }
    }
}

/// Task für den Empfang (verarbeitet eingehende Nachrichten)
#[embassy_executor::task]
pub async fn can_rx_task(mut rx: TwaiRx<'static, Async>) {
    info!("CAN RX Task gestartet");
    loop {
        match rx.receive_async().await {
            Ok(frame) => {
                let id = match frame.id() {
                    Id::Standard(s) => s.as_raw() as u32,
                    Id::Extended(e) => e.as_raw(),
                };
                // let data = frame.data();
                // let dlc = frame.dlc() as usize;

                // Hier findet die J1939-Verarbeitung statt
                // info!("CAN RX: ID={:08X} DLC={} Data={:02X}", id, dlc, &data[..dlc]);

                // J1939 PGN extrahieren (Bits 8-25 der 29-Bit ID)
                let pgn = (id >> 8) & 0x3FFFF;
                let source_address = id & 0xFF;
                info!("RX ID: ID={:08X} PGN: {:05X} von {:02X}", id, pgn, source_address);
                
                // TODO: Eventuell Frames an einen Verarbeitungs-Task weiterleiten
            }
            Err(e) => {
                error!("CAN RX Hardware Fehler: {:?}", e);
                embassy_time::Timer::after_millis(100).await;
            }
        }
    }
}
