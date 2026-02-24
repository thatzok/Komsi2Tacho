use crate::komsi::komsi_task;
use crate::time::{get_current_time_for_j1939, sync_system_time};
use defmt::{debug, error, info, unwrap};
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};
use embedded_can::{Frame, Id};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Io;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::twai::{BaudRate, EspTwaiFrame, ExtendedId, TwaiConfiguration, TwaiMode};
use esp_hal::twai::{TwaiRx, TwaiTx};
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use esp_hal::Async;

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
                info!(
                    "RX ID: ID={:08X} PGN: {:05X} von {:02X}",
                    id, pgn, source_address
                );

                // Check auf die spezifische ID: 0x1CDEEE17
                //   Priority: 7
                //   Data Page (DP): 0
                //   Parameter Group Number (PGN): Reset
                //     Hex: 0xDE00
                //     Dec: 56832
                //   PDU Format: PDU1(222)
                //   Broadcast: false
                //   Destination Address (DA): 0xEE (238)
                //   Source Address (SA): 0x17 (23)
                if id == 0x1CDEEE17 {
                    info!("Spezifischer Request (1CDEEE17) erkannt, sende Antwort...");
                    // Response 0x1CE8FFEE
                    //   Priority: 7
                    //   Data Page (DP): 0
                    //   Parameter Group Number (PGN): AcknowledgmentMessage
                    //    Hex: 0xE800
                    //    Dec: 59392
                    //   PDU Format: PDU1(232)
                    //   Broadcast: true
                    //   Destination Address (DA): 0xFF (255)

                    // ACK
                    // "Ich habe die Anfrage für die PGN 56832 (0xDE00) erhalten und bestätige hiermit
                    // positiv (Control Byte 0), dass ich sie verarbeite."

                    let resp_id = ExtendedId::new(0x1CE8FFEE).unwrap();
                    let resp_data = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0xDE, 0x00];
                    let resp_frame = EspTwaiFrame::new(resp_id, &resp_data).unwrap();

                    // if let Ok(resp_frame) = EspTwaiFrame::new(resp_id, &resp_data) {
                    // Ab in die Warteschlange
                    CAN_TX_CHANNEL.send(resp_frame).await;
                    info!("Antwort-Frame 1CE8FFEE gesendet.");
                    // }
                }

                // TODO: Eventuell Frames an einen weiteren Verarbeitungs-Task weiterleiten
            }
            Err(e) => {
                error!("CAN RX Hardware Fehler: {:?}", e);
                embassy_time::Timer::after_millis(100).await;
            }
        }
    }
}
