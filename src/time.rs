//! Time handling compatible with Yamcs.
//!
//! An *instant* is the number of milliseconds since 1970-01-01T00:00:00 TAI. Unlike the UNIX time,
//! it includes the leap seconds: converting from UNIX (UTC) milliseconds adds TAI-UTC.

/// Value used for missing/invalid times (same as Yamcs)
pub const INVALID_INSTANT: i64 = i64::MIN;

/// instant of the GPS epoch 1980-01-06T00:00:00 UTC
pub const GPS_EPOCH_YAMCS_EPOCH_DELTA: i64 = 315_964_819_000;
/// instant of the TAI epoch 1958-01-01T00:00:00 TAI
pub const TAI_EPOCH_YAMCS_EPOCH_DELTA: i64 = -378_691_200_000;
/// instant of the J2000 epoch 2000-01-01T11:58:55.816 UTC
pub const J2000_EPOCH_YAMCS_EPOCH_DELTA: i64 = 946_727_967_816;

/// (UNIX seconds when it becomes effective, TAI-UTC in seconds)
const LEAP_SECONDS: [(i64, i64); 28] = [
    (63_072_000, 10),    // 1972-01-01
    (78_796_800, 11),    // 1972-07-01
    (94_694_400, 12),    // 1973-01-01
    (126_230_400, 13),   // 1974-01-01
    (157_766_400, 14),   // 1975-01-01
    (189_302_400, 15),   // 1976-01-01
    (220_924_800, 16),   // 1977-01-01
    (252_460_800, 17),   // 1978-01-01
    (283_996_800, 18),   // 1979-01-01
    (315_532_800, 19),   // 1980-01-01
    (362_793_600, 20),   // 1981-07-01
    (394_329_600, 21),   // 1982-07-01
    (425_865_600, 22),   // 1983-07-01
    (489_024_000, 23),   // 1985-07-01
    (567_993_600, 24),   // 1988-01-01
    (631_152_000, 25),   // 1990-01-01
    (662_688_000, 26),   // 1991-01-01
    (709_948_800, 27),   // 1992-07-01
    (741_484_800, 28),   // 1993-07-01
    (773_020_800, 29),   // 1994-07-01
    (820_454_400, 30),   // 1996-01-01
    (867_715_200, 31),   // 1997-07-01
    (915_148_800, 32),   // 1999-01-01
    (1_136_073_600, 33), // 2006-01-01
    (1_230_768_000, 34), // 2009-01-01
    (1_341_100_800, 35), // 2012-07-01
    (1_435_708_800, 36), // 2015-07-01
    (1_483_228_800, 37), // 2017-01-01
];

/// TAI-UTC in seconds at the given UNIX time (in milliseconds).
/// Before 1972 the initial value of 10 seconds is used.
fn tai_utc_at_unix(unix_millis: i64) -> i64 {
    let s = unix_millis.div_euclid(1000);
    LEAP_SECONDS.iter().rev().find(|(t, _)| s >= *t).map_or(LEAP_SECONDS[0].1, |(_, ls)| *ls)
}

/// converts UNIX milliseconds (UTC) to instant
pub fn from_unix_millis(unix_millis: i64) -> i64 {
    unix_millis + 1000 * tai_utc_at_unix(unix_millis)
}

/// converts an instant to UNIX milliseconds (UTC).
/// During a leap second, the time of the beginning of the next UTC day is returned.
pub fn to_unix_millis(instant: i64) -> i64 {
    for &(t, ls) in LEAP_SECONDS.iter().rev() {
        // the instant at which this leap second offset becomes effective
        let start = (t + ls) * 1000;
        if instant >= start {
            return instant - ls * 1000;
        }
        // inside the inserted leap second (the previous offset would give a time after the boundary)
        if instant >= start - 1000 {
            return t * 1000;
        }
    }
    instant - LEAP_SECONDS[0].1 * 1000
}

pub fn from_gps_millis(gps_millis: i64) -> i64 {
    gps_millis + GPS_EPOCH_YAMCS_EPOCH_DELTA
}

pub fn from_tai_millis(tai_millis: i64) -> i64 {
    tai_millis + TAI_EPOCH_YAMCS_EPOCH_DELTA
}

pub fn from_j2000_millis(j2000_millis: i64) -> i64 {
    j2000_millis + J2000_EPOCH_YAMCS_EPOCH_DELTA
}

/// number of days since 1970-01-01 for the given civil date (proleptic Gregorian calendar)
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// civil date (year, month, day) for the number of days since 1970-01-01
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Parses an UTC date or date time and returns the instant.
///
/// Accepted formats: `yyyy-MM-dd`, `yyyy-MM-ddTHH:mm:ss[.SSS][Z]` and `yyyy/DDDTHH:mm:ss[.SSS][Z]`
pub fn parse(s: &str) -> Option<i64> {
    let s = s.trim();
    let s = s.strip_suffix('Z').unwrap_or(s);
    let (date, time) = match s.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (s, None),
    };

    let days = if let Some((y, doy)) = date.split_once('/') {
        let y: i64 = y.parse().ok()?;
        let doy: i64 = doy.parse().ok()?;
        if !(1..=366).contains(&doy) {
            return None;
        }
        days_from_civil(y, 1, 1) + doy - 1
    } else {
        let mut it = date.splitn(3, '-');
        let y: i64 = it.next()?.parse().ok()?;
        let m: u32 = it.next()?.parse().ok()?;
        let d: u32 = it.next()?.parse().ok()?;
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        days_from_civil(y, m, d)
    };

    let mut millis_of_day = 0i64;
    if let Some(time) = time {
        let (hms, frac) = match time.split_once('.') {
            Some((a, b)) => (a, Some(b)),
            None => (time, None),
        };
        let mut it = hms.splitn(3, ':');
        let h: i64 = it.next()?.parse().ok()?;
        let mi: i64 = it.next()?.parse().ok()?;
        let sec: i64 = it.next().unwrap_or("0").parse().ok()?;
        if h > 23 || mi > 59 || sec > 60 {
            return None;
        }
        let ms = match frac {
            Some(f) if !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()) => {
                let f3: String = f.chars().chain("000".chars()).take(3).collect();
                f3.parse::<i64>().ok()?
            }
            Some(_) => return None,
            None => 0,
        };
        millis_of_day = ((h * 60 + mi) * 60 + sec) * 1000 + ms;
    }
    Some(from_unix_millis(days * 86_400_000 + millis_of_day))
}

/// Formats the instant as UTC `yyyy-MM-ddTHH:mm:ss.SSSZ`
pub fn to_string(instant: i64) -> String {
    if instant == INVALID_INSTANT {
        return "INVALID".to_owned();
    }
    let unix = to_unix_millis(instant);
    let days = unix.div_euclid(86_400_000);
    let ms = unix.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y,
        m,
        d,
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1000) % 60,
        ms % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epochs() {
        // GPS epoch is 1980-01-06T00:00:00Z
        assert_eq!(GPS_EPOCH_YAMCS_EPOCH_DELTA, parse("1980-01-06T00:00:00Z").unwrap());
        assert_eq!("1980-01-06T00:00:00.000Z", to_string(from_gps_millis(0)));
        // J2000 is 2000-01-01T11:58:55.816 UTC
        assert_eq!(J2000_EPOCH_YAMCS_EPOCH_DELTA, parse("2000-01-01T11:58:55.816Z").unwrap());
        assert_eq!("2000-01-01T11:58:55.816Z", to_string(from_j2000_millis(0)));
        assert_eq!(from_unix_millis(0), parse("1970-01-01").unwrap());
        assert_eq!(parse("2024-02-29T00:00:00").unwrap(), parse("2024/060T00:00:00").unwrap());
    }

    #[test]
    fn test_leap_seconds() {
        // 2016-12-31T23:59:60 is the leap second; the instants before and after differ by 2 seconds
        let before = parse("2016-12-31T23:59:59Z").unwrap();
        let after = parse("2017-01-01T00:00:00Z").unwrap();
        assert_eq!(2000, after - before);
        assert_eq!("2016-12-31T23:59:59.000Z", to_string(before));
        assert_eq!("2017-01-01T00:00:00.000Z", to_string(after));
        assert_eq!("2017-01-01T00:00:00.000Z", to_string(before + 1000));

        for s in ["1972-01-01T00:00:00.000Z", "1999-12-31T23:59:59.999Z", "2023-06-15T12:34:56.789Z"] {
            assert_eq!(s, to_string(parse(s).unwrap()));
        }
        assert_eq!(1_700_000_000_000 + 37_000, from_unix_millis(1_700_000_000_000));
        assert_eq!(1_700_000_000_000, to_unix_millis(from_unix_millis(1_700_000_000_000)));
    }

    #[test]
    fn test_invalid() {
        assert!(parse("2020-13-01").is_none());
        assert!(parse("abc").is_none());
        assert!(parse("2020-01-01T25:00:00").is_none());
    }
}
