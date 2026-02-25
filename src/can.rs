use crate::komsi::{komsi_task, ACTUAL_SPEED, MAX_SPEED, TOTAL_DISTANCE, TRIP_DISTANCE};
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

use j1939::spn::HighResolutionVehicleDistanceMessage;
use j1939::spn::{DriverTimeRelatedStates, DriverWorkingState, TachographMessage};
use j1939::IdBuilder;
use j1939::PGN;

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

                let j1939_id = j1939::Id::new(id);
                let source_address = id & 0xFF;
                info!(
                    "RX ID: ID={:08X} PGN: {:05X} {:?} von {:02X}",
                    id,
                    pgn,
                    defmt::Debug2Format(&j1939_id.pgn()),
                    source_address
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

pub async fn send_hr_distance_message() {
    // 1. CAN-ID konstruieren (0x18FEC1EE)
    // Priorität: 6 (Standard für Broadcast-PGNs in dieser Range)
    // PGN: High Resolution Vehicle Distance (65217 / 0xFEC1)
    // Source Address: 238 (0xEE)
    let id = IdBuilder::from_pgn(PGN::HighResolutionVehicleDistance)
        .priority(6)
        .sa(0xEE)
        .build();

    let total_dist = TOTAL_DISTANCE.lock(|d| d.get());
    let trip_dist = TRIP_DISTANCE.lock(|d| d.get());

    // 2. High Resolution Vehicle Distance Daten konstruieren (0000000000000000)
    // Dieses Paket enthält zwei Werte mit jeweils 4 Bytes (5 m/Bit Auflösung).
    // Da J1939 hier oft u32 für m verwendet (bis 21M km), passen wir u64 entsprechend an.
    let msg = HighResolutionVehicleDistanceMessage {
        total_vehicle_distance_m: Some(total_dist as u32), // Gesamtfahrstrecke in Metern
        trip_distance_m: Some(trip_dist as u32),           // Tagesfahrstrecke in Metern
    };

    let frame = j1939::FrameBuilder::new(id)
        .copy_from_slice(&msg.to_pdu())
        .build();

    let twai_id = ExtendedId::new(id.as_raw()).unwrap();
    let twai_data = frame.pdu();
    let twai_frame = EspTwaiFrame::new(twai_id, &twai_data).unwrap();
    can_send_frame(twai_frame).await;
    info!("HighResolutionVehicleDistanceMessage gesendet");
}

#[embassy_executor::task]
pub async fn hr_distance_task() {
    loop {
        calculate_distance_per_second();
        send_hr_distance_message().await;
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task]
pub async fn tachograph_task() {
    loop {
        send_tachograph_message().await;
        Timer::after(Duration::from_millis(50)).await;
    }
}
pub async fn send_tachograph_message() {
    let speed = ACTUAL_SPEED.lock(|s| s.get());
    let max_speed = MAX_SPEED.lock(|s| s.get());

    // 1. Konstruiere die ID
    // Priorität 3, PGN Tachograph (65132), Source Address 0xEE
    let id = IdBuilder::from_pgn(PGN::Tachograph)
        .priority(3)
        .sa(0xEE)
        .build();

    // 2. Konstruiere die Tachograph-Nachricht
    // Basierend auf den Daten 00FFFFC100000000
    let msg = TachographMessage {
        driver1_working_state: Some(DriverWorkingState::Drive),
        driver2_working_state: Some(DriverWorkingState::RestSleeping),
        vehicle_motion: Some(speed > 0),
        driver1_time_states: None,
        driver1_card_present: Some(true),
        vehicle_overspeed: Some(max_speed > 0 && speed > max_speed),
        driver2_time_states: None,
        driver2_card_present: None,
        system_event: Some(true),
        handling_information: Some(false),
        tachograph_performance: None,
        direction_indicator: Some(true),
        tachograph_output_shaft_speed: Some(0),
        tachograph_vehicle_speed: Some(speed as u16),
    };

    let frame = j1939::FrameBuilder::new(id)
        .copy_from_slice(&msg.to_pdu())
        .build();

    let twai_id = ExtendedId::new(id.as_raw()).unwrap();
    let twai_data = frame.pdu();
    let twai_frame = EspTwaiFrame::new(twai_id, &twai_data).unwrap();
    can_send_frame(twai_frame).await;

    // info!("TachographMessage gesendet");
}

#[embassy_executor::task]
pub async fn date_time_task() {
    loop {
        send_date_time_message().await;
        Timer::after(Duration::from_secs(1)).await;
    }
}

pub fn calculate_distance_per_second() {
    let speed_kmh = ACTUAL_SPEED.lock(|s| s.get());
    // Formula: meters_per_second = speed_kmh / 3.6
    // To avoid floating point, we can use: meters = speed_kmh * 10 / 36
    let meters_this_second = (speed_kmh as u64 * 10) / 36;

    if meters_this_second > 0 {
        TOTAL_DISTANCE.lock(|d| {
            d.set(d.get().saturating_add(meters_this_second));
        });
        TRIP_DISTANCE.lock(|d| {
            d.set(d.get().saturating_add(meters_this_second));
        });
    }
}
pub async fn send_date_time_message() {
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
    }
}
