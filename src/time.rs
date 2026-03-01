use komsi::KomsiDateTime;
use embassy_time::Instant;

// if we would use AtomicU32 instead of AtomicU64 we would not need "portable_atomic" crate,
// but then we would get a timestamp overflow after 49,7 days. The simulation will probably
// never run this long after a SetTime but I hate possible overflows

// we store the last complete DateTime and a timestamp when it was set
static LAST_DATETIME: spin::RwLock<Option<(KomsiDateTime, Instant)>> = spin::RwLock::new(None);

pub fn sync_system_time(dt: KomsiDateTime) {
    let now = Instant::now();
    let mut lock = LAST_DATETIME.write();
    *lock = Some((dt, now));
    defmt::info!("DateTime set to: {:?}", dt);
}

/// calculates current time based on the last update
pub fn get_current_time_for_j1939() -> Option<KomsiDateTime> {
    let lock = LAST_DATETIME.read();
    let (base_dt, base_instant) = (*lock)?;

    let elapsed_secs = Instant::now().duration_since(base_instant).as_secs();
    if elapsed_secs == 0 {
        return Some(base_dt);
    }

    // we add the seconds since last update to the DateTime
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
        // this is ok for small time spans (minutes, hours) because we will receive the setTime
        // KOMSI-command usually at least once a day
        // for longer timestamps (several days) we would need a real calendar function
        dt.day += days_to_add as u8;
    }
}
