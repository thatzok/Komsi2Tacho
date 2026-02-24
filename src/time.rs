use portable_atomic::{AtomicU64, Ordering};
use embassy_time::Instant;
use crate::komsi::KomsiDateTime;

// Wir speichern das letzte komplette Datum und wann es empfangen wurde
static LAST_DATETIME: spin::RwLock<Option<(KomsiDateTime, Instant)>> = spin::RwLock::new(None);

pub fn sync_system_time(dt: KomsiDateTime) {
    let now = Instant::now();
    let mut lock = LAST_DATETIME.write();
    *lock = Some((dt, now));
    defmt::info!("Zeit-Referenzpunkt gesetzt: {:?}", dt);
}

/// Berechnet die aktuelle Zeit basierend auf dem letzten KOMSI-Update
pub fn get_current_time_for_j1939() -> Option<KomsiDateTime> {
    let lock = LAST_DATETIME.read();
    let (base_dt, base_instant) = (*lock)?;

    let elapsed_secs = Instant::now().duration_since(base_instant).as_secs();
    if elapsed_secs == 0 {
        return Some(base_dt);
    }

    // Wir addieren die vergangenen Sekunden auf das Datum
    let mut current = base_dt;
    add_seconds(&mut current, elapsed_secs);
    Some(current)
}

fn add_seconds(dt: &mut KomsiDateTime, secs: u64) {
    let total_secs = dt.sec as u64 + secs;
    dt.sec = (total_secs % 60) as u8;

    let total_mins = dt.min as u64 + (total_secs / 60);
    dt.min = (total_mins % 60) as u8;

    let total_hours = dt.hour as u64 + (total_mins / 60);
    dt.hour = (total_hours % 24) as u8;

    let days_to_add = total_hours / 24;
    if days_to_add > 0 {
        // Für kurze Zeiträume (Minuten/Stunden) reicht das.
        // Für Tage bräuchte man einen echten Kalender-Addierer.
        dt.day += days_to_add as u8;
    }
}
