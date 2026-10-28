// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Arayüzde kullanılan saf biçimlendirme yardımcıları.
//!
//! Buradaki fonksiyonların egui'ye bağımlılığı yoktur; bu nedenle birim
//! testleriyle doğrulanabilirler. Para birimi biçimlendirmesi iş mantığı
//! katmanındadır (bkz. [`crate::money`]); burası yalnızca görüntülenecek
//! metni üretir.

use chrono::{Datelike, NaiveDate};

/// Türkçe ay adları (Ocak = 1).
const MONTH_NAMES: [&str; 12] = [
    "Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim",
    "Kasım", "Aralık",
];

/// Türkçe gün adları (Pazartesi = 0).
const WEEKDAY_NAMES: [&str; 7] = [
    "Pazartesi",
    "Salı",
    "Çarşamba",
    "Perşembe",
    "Cuma",
    "Cumartesi",
    "Pazar",
];

/// Uzun Türkçe tarih: `18 Ekim 2026 Pazar`.
pub fn format_date(date: NaiveDate) -> String {
    format!(
        "{} {} {} {}",
        date.day(),
        month_name(date),
        date.year(),
        weekday_name(date)
    )
}

/// Kısa Türkçe tarih: `18.10.2026`.
pub fn format_date_short(date: NaiveDate) -> String {
    format!("{:02}.{:02}.{:04}", date.day(), date.month(), date.year())
}

/// ISO biçimli (`YYYY-MM-DD`) tarih metnini kısa Türkçe biçime çevirir.
///
/// Metin geçerli bir tarih değilse olduğu gibi döndürülür; böylece eksik/geçersiz
/// bir değer arayüzde kaybolmaz.
pub fn format_iso_date_short(value: &str) -> String {
    match NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d") {
        Ok(date) => format_date_short(date),
        Err(_) => value.to_owned(),
    }
}

/// Bayt cinsinden boyutu okunabilir biçime çevirir (örneğin `2.0 KB`).
///
/// Buradaki `f64` yalnızca dosya boyutu gösterimi içindir; para tutarları
/// hiçbir zaman kayan noktalı sayı ile hesaplanmaz (bkz. [`crate::money`]).
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Ay adı (Türkçe).
fn month_name(date: NaiveDate) -> &'static str {
    MONTH_NAMES[(date.month() - 1) as usize]
}

/// Gün adı (Türkçe).
fn weekday_name(date: NaiveDate) -> &'static str {
    WEEKDAY_NAMES[date.weekday().num_days_from_monday() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tarih(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("geçerli bir tarih olmalı")
    }

    #[test]
    fn turkce_uzun_tarih_dogru_bicimlenir() {
        assert_eq!(format_date(tarih(2026, 10, 18)), "18 Ekim 2026 Pazar");
        assert_eq!(format_date(tarih(2026, 1, 5)), "5 Ocak 2026 Pazartesi");
        assert_eq!(format_date(tarih(2026, 6, 1)), "1 Haziran 2026 Pazartesi");
        assert_eq!(format_date(tarih(2024, 2, 29)), "29 Şubat 2024 Perşembe");
    }

    #[test]
    fn turkce_kisa_tarih_dogru_bicimlenir() {
        assert_eq!(format_date_short(tarih(2026, 10, 18)), "18.10.2026");
        assert_eq!(format_date_short(tarih(2026, 1, 5)), "05.01.2026");
        assert_eq!(format_date_short(tarih(2026, 12, 31)), "31.12.2026");
    }

    #[test]
    fn iso_tarih_metni_kisa_bicime_cevrilir() {
        assert_eq!(format_iso_date_short("2026-10-18"), "18.10.2026");
        assert_eq!(format_iso_date_short(" 2026-01-05 "), "05.01.2026");
        // Geçersiz metin olduğu gibi kalır.
        assert_eq!(format_iso_date_short("bilinmiyor"), "bilinmiyor");
        assert_eq!(format_iso_date_short(""), "");
    }

    #[test]
    fn bayt_bicimlendirme() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    }
}
