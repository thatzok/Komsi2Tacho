use crate::commands::{
    ACTUAL_SPEED, CAN_STATUS, CanStatus, LedSignal, MAX_SPEED, TOTAL_DISTANCE, TRIP_DISTANCE,
    set_led_signal, usb_write, usb_write_dynamic,
};
use crate::time::get_current_time_for_j1939;
use alloc::format;
use core::fmt::Write as _;
use defmt::{error, info, warn};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use embedded_can::{Frame, Id};
use esp_hal::Async;
use esp_hal::twai::{EspTwaiFrame, ExtendedId, Twai};
use heapless::String;

use j1939::IdBuilder;
use j1939::PGN;
use j1939::spn::{AcknowledgmentMessage, AcknowledgmentType, HighResolutionVehicleDistanceMessage};
use j1939::spn::{DriverWorkingState, TachographMessage};

// Channel for 16 frames - buffer for "sending everything once"
// 16 frames is more than enough for our use case
pub static CAN_TX_CHANNEL: Channel<CriticalSectionRawMutex, EspTwaiFrame, 16> = Channel::new();

#[embassy_executor::task]
pub async fn can_manager_task(mut twai: Twai<'static, Async>) {
    info!("CAN Manager Task started (Combined TX/RX)");

    loop {
        // we save result of select in a variable
        let selected =
            embassy_futures::select::select(CAN_TX_CHANNEL.receive(), twai.receive_async()).await;

        // after  .await is finished the Borrows of 'select' have endet
        // and wen can use 'twai' again

        match selected {
            // sending
            embassy_futures::select::Either::First(frame) => {
                match embassy_time::with_timeout(
                    Duration::from_millis(100),
                    twai.transmit_async(&frame),
                )
                .await
                {
                    Ok(Ok(_)) => {
                        // success
                        CAN_STATUS.lock(|s| *s.borrow_mut() = CanStatus::Ready);
                    }
                    Ok(Err(e)) => {
                        error!("CAN TX Error: {:?}", e);
                        // let mut s: String<64> = String::new();
                        // let _ = write!(s, "CAN TX Error: {:?}", e);
                        // usb_write_dynamic(s);

                        if format!("{:?}", e).contains("BusOff") {
                            CAN_STATUS.lock(|s| *s.borrow_mut() = CanStatus::BusOff);
                            warn!("CAN-Bus-Off detected! Resetting...");

                            let cfg = twai.stop();
                            Timer::after(Duration::from_millis(1000)).await;
                            twai = cfg.start();
                            info!("Controller restarted.");
                        } else {
                            CAN_STATUS.lock(|s| {
                                let mut err_msg: String<32> = String::new();
                                let _ = write!(err_msg, "{:?}", e);
                                *s.borrow_mut() = CanStatus::TransmitError(err_msg);
                            });
                        }
                    }
                    Err(e) => {
                        CAN_STATUS.lock(|s| {
                            let mut err_msg: String<32> = String::new();
                            let _ = write!(err_msg, "{:?}", e);
                            *s.borrow_mut() = CanStatus::OtherError(err_msg);
                        });
                        warn!("CAN TX Timeout! Controller might be stuck, resetting...");
                        // let mut s: String<64> = String::new();
                        // let _ = write!(s, "CAN TX Error: {:?}", e);
                        // usb_write_dynamic(s);

                        let cfg = twai.stop();
                        Timer::after(Duration::from_millis(1000)).await;
                        twai = cfg.start();
                        info!("Controller restarted after.");
                    }
                }
            }

            // we should reveive
            embassy_futures::select::Either::Second(result) => {
                match result {
                    Ok(frame) => {
                        CAN_STATUS.lock(|s| *s.borrow_mut() = CanStatus::Ready);
                        let id = match frame.id() {
                            Id::Standard(s) => s.as_raw() as u32,
                            Id::Extended(e) => e.as_raw(),
                        };

                        // Extract J1939 PGN (Bits 8-25 of the 29-bit ID)
                        // just for info message
                        let pgn = (id >> 8) & 0x3FFFF;
                        let j1939_id = j1939::Id::new(id);
                        let source_address = id & 0xFF;
                        info!(
                            "RX ID: ID={:08X} PGN: {:05X} {:?} from {:02X}",
                            id,
                            pgn,
                            defmt::Debug2Format(&j1939_id.pgn()),
                            source_address
                        );

                        // Check for specific ID: 0x1CDEEE17
                        // PGN 56832 Reset from Source Address 0x17
                        if id == 0x1CDEEE17 {
                            info!(
                                "Specific RESET request (1CDEEE17) detected, sending response..."
                            );
                            send_acknowledgment_message().await;
                        }
                    }
                    Err(e) => {
                        CAN_STATUS.lock(|s| {
                            let mut err_msg: String<32> = String::new();
                            let _ = write!(err_msg, "{:?}", e);
                            *s.borrow_mut() = CanStatus::ReceiveError(err_msg);
                        });
                        error!("CAN RX hardware error: {:?}", e);
                        let mut s: String<64> = String::new();
                        let _ = write!(s, "ERR: CAN RX Error: {:?}", e);
                        usb_write_dynamic(s);
                        embassy_time::Timer::after_millis(100).await;
                    }
                }
            }
        }
    }
}

/// Helper function to put a packet into the queue from anywhere
pub async fn can_send_frame(frame: EspTwaiFrame) {
    CAN_TX_CHANNEL.send(frame).await;
}

/// Helper function for local loopback test mode
#[embassy_executor::task]
pub async fn can_self_test_task(mut twai: Twai<'static, Async>) {
    info!("CAN Self-Test Task started");
    usb_write("--- CAN-Bus Self-Test Mode ---");

    let test_id = ExtendedId::new(0x1234567).unwrap();
    let test_data = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];
    // let test_frame = EspTwaiFrame::new(test_id, &test_data).unwrap();
    let test_frame = EspTwaiFrame::new_self_reception(test_id, &test_data).unwrap();

    loop {
        // Send test frame
        let send_result = embassy_time::with_timeout(
            Duration::from_millis(100),
            twai.transmit_async(&test_frame),
        )
        .await;

        match send_result {
            Ok(_) => {
                info!("Testframe gesendet");
                // Wait for the frame to be received (SelfTest mode receives its own frames)
                match embassy_time::with_timeout(Duration::from_millis(100), twai.receive_async())
                    .await
                {
                    Ok(Ok(rx_frame)) => {
                        info!("Testframe empfangen");
                        if rx_frame.id() == test_frame.id() && rx_frame.data() == test_frame.data()
                        {
                            usb_write("Selbsttest erfolgreich: Paket korrekt empfangen");
                        } else {
                            let mut msg: String<64> = String::new();
                            let _ = write!(
                                msg,
                                "Fehler: Paketinhalt falsch (ID: {:X?})",
                                rx_frame.id()
                            );
                            usb_write_dynamic(msg);
                        }
                    }
                    Ok(Err(e)) => {
                        error!("Fehler beim Empfang: {:?}", e);
                        let mut msg: String<64> = String::new();
                        let _ = write!(msg, "Fehler beim Empfang: {:?}", e);
                        usb_write_dynamic(msg);
                    }
                    Err(_) => {
                        error!("Timeout beim Empfang");
                        usb_write("Fehler: Timeout beim Empfang");
                    }
                }
            }
            Err(e) => {
                info!("Fehler beim Senden: {:?}", e);
                let mut msg: String<64> = String::new();
                let _ = write!(msg, "Fehler beim Senden: {:?}", e);
                usb_write_dynamic(msg);
            }
        }

        Timer::after(Duration::from_secs(1)).await;
    }
}

pub async fn send_acknowledgment_message() {
    // PGN: Acknowledgment (0xE800 = 59392)
    // Source Address: 0xEE

    let id = IdBuilder::from_pgn(PGN::AcknowledgmentMessage)
        .priority(7)
        .da(0xFF)
        .sa(0xEE)
        .build();

    let msg = AcknowledgmentMessage {
        control_byte: Some(AcknowledgmentType::Positive),
        group_function_value: 0xFF,
        pgn: PGN::from(0x00DE00),
    };

    let frame = j1939::FrameBuilder::new(id)
        .copy_from_slice(&msg.to_pdu())
        .build();

    let twai_id = ExtendedId::new(id.as_raw()).unwrap();
    let twai_data = frame.pdu();
    let twai_frame = EspTwaiFrame::new(twai_id, &twai_data).unwrap();
    if let Err(_) = CAN_TX_CHANNEL.try_send(twai_frame) {
        warn!("AcknowledgmentMessage dropped (channel full)");
    } else {
        info!("AcknowledgmentMessage sent");
    }
}

pub async fn send_hr_distance_message() {
    // PGN: High Resolution Vehicle Distance (65217 / 0xFEC1)
    // Source Address: 238 (0xEE)
    let id = IdBuilder::from_pgn(PGN::HighResolutionVehicleDistance)
        .priority(6)
        .sa(0xEE)
        .build();

    let total_dist = TOTAL_DISTANCE.lock(|d| d.get());
    let trip_dist = TRIP_DISTANCE.lock(|d| d.get());

    // Since J1939 often uses u32 for m here (up to 21M km), we adapt u64 accordingly.
    let msg = HighResolutionVehicleDistanceMessage {
        total_vehicle_distance_m: Some(total_dist as u32), // Total distance in meters
        trip_distance_m: Some(trip_dist as u32),           // Trip distance in meters
    };

    let frame = j1939::FrameBuilder::new(id)
        .copy_from_slice(&msg.to_pdu())
        .build();

    let twai_id = ExtendedId::new(id.as_raw()).unwrap();
    let twai_data = frame.pdu();
    let twai_frame = EspTwaiFrame::new(twai_id, &twai_data).unwrap();
    can_send_frame(twai_frame).await;
    info!("HighResolutionVehicleDistanceMessage sent");
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
        Timer::after(Duration::from_millis(45)).await; // regular 50 milliseconds, but we use some security margin
    }
}

pub async fn send_tachograph_message() {
    // PGN Tachograph (65132), Source Address 0xEE

    let speed = ACTUAL_SPEED.lock(|s| s.get());
    let max_speed = MAX_SPEED.lock(|s| s.get());

    let id = IdBuilder::from_pgn(PGN::Tachograph)
        .priority(3)
        .sa(0xEE)
        .build();

    let msg = TachographMessage {
        driver1_working_state: Some(DriverWorkingState::Drive),
        driver2_working_state: None, // Some(DriverWorkingState::RestSleeping),

        // IMPORTANT: Must be 'true' if the vehicle is moving
        vehicle_motion: Some(speed > 0),

        driver1_time_states: None,
        driver1_card_present: Some(true),

        // Speed in km/h, max_speed in km/h
        vehicle_overspeed: Some(max_speed > 0 && speed > max_speed),

        driver2_time_states: None,
        driver2_card_present: Some(false), // Better 'false' instead of 'None'

        system_event: Some(false), // 'false' usually means "No Event", 'true' could trigger a warning lamp

        handling_information: Some(false), // normal operation, no one is in the setup of the "Fahrtenschreiber"

        tachograph_performance: Some(false), //   "Fahrtenschreiber" is working without error

        direction_indicator: Some(true), // true = Forward

        // IMPORTANT: Must not be 0 if the vehicle speed >0
        // The 1323/1324 compares both values. If the shaft stops but the vehicle moves, the tacho
        // assumes manipulation (magnet on sensor) and indicates a fault.
        //
        // We simulate a plausible value with a k-value of 8000 imp/km
        tachograph_output_shaft_speed: Some(((speed as f32) * 133.3) as u16),

        tachograph_vehicle_speed: Some(speed as u16),
    };

    let frame = j1939::FrameBuilder::new(id)
        .copy_from_slice(&msg.to_pdu())
        .build();

    let twai_id = ExtendedId::new(id.as_raw()).unwrap();
    let twai_data = frame.pdu();
    let twai_frame = EspTwaiFrame::new(twai_id, &twai_data).unwrap();
    can_send_frame(twai_frame).await;
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
    // To avoid floating point for precision, we can use: meters = speed_kmh * 10 / 36
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
        info!("Cyclical TimeDate sent");
    }
}
