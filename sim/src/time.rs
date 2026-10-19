//! Tempo da simulação: segundos desde J2000.0 (1º de janeiro de 2000, 12:00).
//!
//! A diferença entre TT e UTC (pouco mais de um minuto) é ignorada: o mesmo
//! relógio serve às efemérides e ao relógio de parede.

use crate::math::{TAU, wrap_2pi};

pub const DAY: f64 = 86_400.0;
/// Data juliana de J2000.0.
pub const JD_J2000: f64 = 2_451_545.0;

/// Data juliana de um instante.
pub fn julian_date(t: f64) -> f64 {
    JD_J2000 + t / DAY
}

/// Séculos julianos desde J2000.
pub fn centuries(t: f64) -> f64 {
    t / (DAY * 36_525.0)
}

/// Tempo sideral médio de Greenwich (rad): o quanto a Terra girou em relação ao
/// equinócio. Fórmula da IAU 1982, boa a fração de segundo nestas décadas.
pub fn gmst(t: f64) -> f64 {
    let d = t / DAY;
    let tc = d / 36_525.0;
    let deg = 280.460_618_37 + 360.985_647_366_29 * d + 0.000_387_933 * tc * tc
        - tc * tc * tc / 38_710_000.0;
    wrap_2pi(deg.to_radians())
}

/// Instante a partir da data civil (UTC).
pub fn from_calendar(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: f64) -> f64 {
    // Dias desde 2000-01-01 pelo algoritmo de dias civis (Howard Hinnant).
    let days = days_from_civil(year, month, day) - days_from_civil(2000, 1, 1);
    days as f64 * DAY + (hour as f64 - 12.0) * 3600.0 + minute as f64 * 60.0 + second
}

/// Data civil (ano, mês, dia, hora, minuto, segundo) de um instante.
pub fn to_calendar(t: f64) -> (i32, u32, u32, u32, u32, f64) {
    let since_midnight = t + 12.0 * 3600.0;
    let days = (since_midnight / DAY).floor();
    let mut secs = since_midnight - days * DAY;
    let (y, m, d) = civil_from_days(days as i64 + days_from_civil(2000, 1, 1));
    let h = (secs / 3600.0).floor();
    secs -= h * 3600.0;
    let mi = (secs / 60.0).floor();
    secs -= mi * 60.0;
    (y, m, d, h as u32, mi as u32, secs)
}

/// "2026-10-19 14:05:09 UTC"
pub fn format_utc(t: f64) -> String {
    let (y, m, d, h, mi, s) = to_calendar(t);
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{:02} UTC", s.floor() as u32)
}

/// Lê "AAAA-MM-DD HH:MM:SS" (ou com T, com ou sem segundos e "Z"/" UTC").
pub fn parse_utc(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches("UTC").trim().trim_end_matches('Z');
    let (date, time) = match s.split_once(['T', ' ']) {
        Some((d, t)) => (d, t),
        None => (s, "00:00:00"),
    };
    let mut dp = date.split('-');
    let y: i32 = dp.next()?.parse().ok()?;
    let mo: u32 = dp.next()?.parse().ok()?;
    let d: u32 = dp.next()?.parse().ok()?;
    let mut tp = time.split(':');
    let h: u32 = tp.next()?.parse().ok()?;
    let mi: u32 = tp.next().unwrap_or("0").parse().ok()?;
    let sec: f64 = tp.next().unwrap_or("0").parse().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec >= 61.0 {
        return None;
    }
    Some(from_calendar(y, mo, d, h, mi, sec))
}

fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

/// "T-00:09:58", "T+00:02:31" ou "T+3d 04:00:12" para tempos longos.
pub fn format_met(t: f64) -> String {
    let sign = if t < 0.0 { '-' } else { '+' };
    let a = t.abs();
    let days = (a / DAY).floor();
    let rest = a - days * DAY;
    let h = (rest / 3600.0).floor();
    let m = ((rest - h * 3600.0) / 60.0).floor();
    let s = (rest - h * 3600.0 - m * 60.0).floor();
    if days > 0.0 {
        format!("T{sign}{days}d {h:02}:{m:02}:{s:02}")
    } else {
        format!("T{sign}{h:02}:{m:02}:{s:02}")
    }
}

/// Rotação da Terra (rad/s) em relação às estrelas.
pub const EARTH_ROTATION: f64 = TAU / 86_164.090_5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendario_ida_e_volta() {
        let t = from_calendar(2026, 10, 19, 14, 5, 9.0);
        let (y, m, d, h, mi, s) = to_calendar(t);
        assert_eq!((y, m, d, h, mi), (2026, 10, 19, 14, 5));
        assert!((s - 9.0).abs() < 1e-6);
        assert_eq!(from_calendar(2000, 1, 1, 12, 0, 0.0), 0.0);
        assert_eq!(parse_utc("2026-10-19T14:05:09Z"), Some(t));
        assert_eq!(format_utc(t), "2026-10-19 14:05:09 UTC");
    }

    #[test]
    fn gmst_em_j2000() {
        // Em J2000 o GMST é 280,46°.
        assert!((gmst(0.0).to_degrees() - 280.460_618_37).abs() < 1e-6);
    }

    #[test]
    fn met() {
        assert_eq!(format_met(-598.0), "T-00:09:58");
        assert_eq!(format_met(151.2), "T+00:02:31");
    }
}
