// Nutze portable_atomic anstelle von core::sync::atomic
use portable_atomic::{AtomicI64, Ordering};
use embassy_time::Instant;
use crate::komsi::KomsiDateTime;

// Globaler Speicher für den Boot-Zeitpunkt (Unix Epoch)
static BOOT_TIME_UNIX: AtomicI64 = AtomicI64::new(0);

/// Setzt die Systemzeit basierend auf einem KOMSI-Datum
pub fn sync_system_time(dt: &KomsiDateTime) {
    let unix_now = calculate_unix_seconds(dt);
    let uptime_secs = Instant::now().as_secs() as i64;

    // Berechne den fiktiven Unix-Zeitpunkt des Bootvorgangs
    BOOT_TIME_UNIX.store(unix_now - uptime_secs, Ordering::SeqCst);

    defmt::info!("Systemzeit synchronisiert auf: {}", dt);
}

/// Gibt den aktuellen Unix-Zeitstempel (Sekunden seit 1970) zurück
pub fn get_now_unix() -> i64 {
    let boot_time = BOOT_TIME_UNIX.load(Ordering::SeqCst);
    if boot_time == 0 {
        return 0; // Zeit wurde noch nicht gesetzt
    }
    boot_time + (Instant::now().as_secs() as i64)
}

/// Hilfsfunktion: Berechnet Unix-Sekunden aus KomsiDateTime (ohne externe Library)
fn calculate_unix_seconds(dt: &KomsiDateTime) -> i64 {
    let y = dt.year as i64;
    let m = dt.month as i64;
    let d = dt.day as i64;

    // Unix-Zeit Berechnung (vereinfacht für Jahre >= 1970)
    let mut days = (y - 1970) * 365 + (y - 1969) / 4;

    // Tage der Monate (Normaljahr)
    let month_days = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for i in 1..(m as usize) {
        days += month_days[i] as i64;
    }

    // Schalttag-Korrektur für das aktuelle Jahr
    if m > 2 && y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
        days += 1;
    }

    days += d - 1;

    let total_secs = days * 86400
        + (dt.hour as i64 * 3600)
        + (dt.min as i64 * 60)
        + (dt.sec as i64);

    total_secs
}
