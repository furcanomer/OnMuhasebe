// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! CSV okuma/yazma yardımcıları.
//!
//! Bu modül **saf biçim mantığıdır**: veritabanına, arayüze veya dosya
//! seçicisine bağımlı değildir. Dışa aktarmada Türkçe Excel ile uyumlu biçim
//! (UTF-8 BOM, `;` ayırıcı, `,` ondalık) üretir; içe aktarmada ise BOM'lu veya
//! BOM'suz UTF-8 metni ve tırnaklı alanları çözer.
//!
//! Para ve miktar biçimleri bilinçli olarak **binlik ayırıcı içermez**:
//! `1234,56` ve `12,5`. Böylece farklı Excel yerel ayarlarında sayı olarak
//! algılanma olasılığı yükselir.

use std::path::Path;

use crate::error::{AppError, Result};
use crate::money::Money;
use crate::quantity::Quantity;

/// UTF-8 BOM (Excel'in kodlamayı doğru algılaması için).
pub const BOM: char = '\u{feff}';

/// Dışa aktarılan CSV dosyasında kullanılan ayırıcı.
pub const DELIMITER: char = ';';

/// CSV hücresini gerekiyorsa tırnaklar.
fn escape_field(value: &str) -> String {
    let needs_quotes = value.contains(DELIMITER)
        || value.contains('"')
        || value.contains('\r')
        || value.contains('\n');
    if needs_quotes {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Satırları BOM'lu UTF-8 CSV olarak yazar.
pub fn write_rows(path: impl AsRef<Path>, rows: &[Vec<String>]) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| AppError::file_system(parent, error))?;
    }

    let mut content = String::with_capacity(rows.len() * 64 + 1);
    content.push(BOM);
    for row in rows {
        for (index, field) in row.iter().enumerate() {
            if index > 0 {
                content.push(DELIMITER);
            }
            content.push_str(&escape_field(field));
        }
        content.push_str("\r\n");
    }

    std::fs::write(path, content).map_err(|error| AppError::file_system(path, error))
}

/// Para tutarını CSV için yazar (`1234,56`, negatifse `-1234,56`).
pub fn money(value: Money) -> String {
    let negative = value.kurus() < 0;
    let absolute = value.kurus().unsigned_abs();
    let lira = absolute / Money::KURUS_PER_LIRA as u64;
    let kurus = absolute % Money::KURUS_PER_LIRA as u64;
    let sign = if negative { "-" } else { "" };
    format!("{sign}{lira},{kurus:02}")
}

/// Miktarı CSV için yazar; gereksiz ondalık sıfırları atar (`12,5`).
pub fn quantity(value: Quantity) -> String {
    let negative = value.milli() < 0;
    let absolute = value.milli().unsigned_abs();
    let units = absolute / Quantity::MILLI_PER_UNIT as u64;
    let milli = (absolute % Quantity::MILLI_PER_UNIT as u64) as u32;
    let sign = if negative { "-" } else { "" };

    if milli == 0 {
        format!("{sign}{units}")
    } else {
        let mut fraction = format!("{milli:03}");
        while fraction.ends_with('0') {
            fraction.pop();
        }
        format!("{sign}{units},{fraction}")
    }
}

/// Mantıksal değeri Türkçe CSV metnine çevirir.
pub fn boolean(value: bool) -> &'static str {
    if value { "Evet" } else { "Hayır" }
}

/// `None` değerini boş hücreye çevirir.
pub fn optional(value: Option<&str>) -> String {
    value.unwrap_or_default().to_owned()
}

/// Ayrıştırılmış CSV tablosu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvTable {
    /// Kullanılan ayırıcı (`;` veya `,`).
    pub delimiter: char,
    /// Satırlar; başlık satırı da dâhildir.
    pub rows: Vec<Vec<String>>,
}

/// CSV metnini ayrıştırır.
///
/// İlk satırda hem `;` hem `,` varsa daha çok geçen ayırıcı seçilir. Tırnaklı
/// alanlar, `""` kaçışı ve CRLF/LF satır sonları desteklenir. Tamamen boş
/// satırlar atlanır. BOM varsa ilk hücreden temizlenir.
pub fn parse(text: &str) -> std::result::Result<CsvTable, String> {
    let text = text.strip_prefix(BOM).unwrap_or(text);
    let delimiter = detect_delimiter(text);

    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();

    while let Some(character) = chars.next() {
        if in_quotes {
            if character == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(character);
            }
            continue;
        }

        match character {
            '"' => in_quotes = true,
            character if character == delimiter => {
                row.push(std::mem::take(&mut field));
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(character),
        }
    }

    if in_quotes {
        return Err("CSV dosyasında kapanmamış tırnak var.".to_owned());
    }

    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }

    rows.retain(|row| row.iter().any(|field| !field.trim().is_empty()));
    if rows.is_empty() {
        return Err("CSV dosyası boş.".to_owned());
    }

    Ok(CsvTable { delimiter, rows })
}

/// İlk satırdaki ayırıcıyı belirler.
fn detect_delimiter(text: &str) -> char {
    let first_line = text.lines().next().unwrap_or_default();
    let semicolons = first_line.chars().filter(|&c| c == ';').count();
    let commas = first_line.chars().filter(|&c| c == ',').count();

    match (semicolons, commas) {
        (0, 0) => DELIMITER,
        (0, _) => ',',
        (_, 0) => ';',
        (semicolons, commas) => {
            if commas > semicolons {
                ','
            } else {
                ';'
            }
        }
    }
}

/// CSV içe aktarmada miktar çözer.
///
/// Para çözümleyicisiyle aynı mantık kullanılır, fakat miktar üç ondalık
/// basamağa kadar desteklenir. Yalnızca nokta varsa ve ardından 1-3 hane
/// geliyorsa nokta ondalık kabul edilir (`12.5`); aksi hâlde binliktir.
pub fn parse_quantity_import(input: &str) -> std::result::Result<Quantity, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Quantity::ZERO);
    }

    let (sign, body) = match trimmed.strip_prefix('-') {
        Some(rest) => (-1_i64, rest),
        None => (1_i64, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };

    let mut cleaned = String::with_capacity(body.len());
    for character in body.chars() {
        match character {
            '0'..='9' | '.' | ',' => cleaned.push(character),
            ' ' | '\u{00A0}' | '\u{202F}' | '\'' | '_' => {}
            other if other.is_alphabetic() => {}
            _ => return Err(format!("Geçersiz miktar: '{input}'")),
        }
    }
    if cleaned.is_empty() {
        return Err(format!("Geçersiz miktar: '{input}'"));
    }

    let dot = cleaned.rfind('.');
    let comma = cleaned.rfind(',');
    let decimal_index = match (dot, comma) {
        (Some(dot), Some(comma)) => Some(dot.max(comma)),
        (None, Some(comma)) => Some(comma),
        (Some(dot), None) => {
            let after = &cleaned[dot + 1..];
            let digits = after.chars().filter(char::is_ascii_digit).count();
            if cleaned.matches('.').count() == 1
                && after.chars().all(|c| c.is_ascii_digit())
                && (1..=Quantity::FRACTION_MAX_LEN).contains(&digits)
            {
                Some(dot)
            } else {
                None
            }
        }
        (None, None) => None,
    };

    let (integer_text, fraction_text) = match decimal_index {
        Some(index) => (&cleaned[..index], &cleaned[index + 1..]),
        None => (cleaned.as_str(), ""),
    };

    let thousands = if let Some(index) = decimal_index {
        let separator = cleaned.as_bytes()[index] as char;
        if cleaned
            .matches(if separator == '.' { ',' } else { '.' })
            .count()
            > 0
        {
            Some(if separator == '.' { ',' } else { '.' })
        } else {
            None
        }
    } else if cleaned.matches('.').count() > 0 {
        Some('.')
    } else if cleaned.matches(',').count() > 0 {
        Some(',')
    } else {
        None
    };

    let integer_digits = decimal_integer_digits(integer_text, thousands, input)?;
    if !fraction_text.chars().all(|c| c.is_ascii_digit())
        || fraction_text.len() > Quantity::FRACTION_MAX_LEN
    {
        return Err(format!("Geçersiz miktar: '{input}'"));
    }

    let units = parse_digits(&integer_digits, input)?;
    let mut fraction = fraction_text.to_owned();
    while fraction.len() < Quantity::FRACTION_MAX_LEN {
        fraction.push('0');
    }
    let milli = parse_digits(&fraction, input)?;
    let total = units
        .checked_mul(Quantity::MILLI_PER_UNIT)
        .and_then(|value| value.checked_add(milli))
        .ok_or_else(|| "Miktar çok büyük.".to_owned())?;

    Ok(Quantity::from_milli(sign * total))
}

/// Tam kısmı doğrular ve yalnızca rakamları döndürür.
fn decimal_integer_digits(
    text: &str,
    thousands: Option<char>,
    original: &str,
) -> std::result::Result<String, String> {
    if text.is_empty() {
        return Ok(String::new());
    }
    let allowed = |character: char| character.is_ascii_digit() || Some(character) == thousands;
    if !text.chars().all(allowed) {
        return Err(format!("Geçersiz miktar: '{original}'"));
    }
    if let Some(separator) = thousands {
        for (index, group) in text.split(separator).enumerate() {
            let length = group.chars().count();
            let valid = if index == 0 {
                (1..=3).contains(&length)
            } else {
                length == 3
            };
            if !valid {
                return Err(format!("Geçersiz miktar: '{original}'"));
            }
        }
    }
    Ok(text.chars().filter(char::is_ascii_digit).collect())
}

/// Rakam grubunu `i64` değerine çevirir.
fn parse_digits(digits: &str, original: &str) -> std::result::Result<i64, String> {
    if digits.is_empty() {
        return Ok(0);
    }
    digits
        .parse::<i64>()
        .map_err(|_| format!("Geçersiz miktar: '{original}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_bom_ayirici_ve_ondalik_bicimi_dogru_yazilir() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("deneme.csv");
        write_rows(
            &path,
            &[
                vec!["Ad".to_owned(), "Tutar".to_owned()],
                vec!["Çağrı".to_owned(), money(Money::from_kurus(123_456))],
                vec!["Miktar".to_owned(), quantity(Quantity::from_milli(12_500))],
            ],
        )
        .unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("Çağrı;1234,56"));
        assert!(text.contains("Miktar;12,5"));
        assert!(text.contains("\r\n"));
    }

    #[test]
    fn csv_ozel_karakterler_tirnaklanir() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("deneme.csv");
        write_rows(&path, &[vec!["a;b".to_owned(), "tırnak \" var".to_owned()]]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"a;b\";\"tırnak \"\" var\""));
    }

    #[test]
    fn csv_tirnakli_alanlar_ve_crlf_okunur() {
        let table = parse("\u{feff}code;name\r\nA;\"Bu; bir \"\"ad\"\"\"\r\nB;İkinci\r\n").unwrap();
        assert_eq!(table.delimiter, ';');
        assert_eq!(table.rows.len(), 3);
        assert_eq!(table.rows[1][1], "Bu; bir \"ad\"");
        assert_eq!(table.rows[2][1], "İkinci");
    }

    #[test]
    fn csv_ayirici_ilk_satirda_daha_cok_gecen_olur() {
        let table = parse("code,name\nA;B\n").unwrap();
        assert_eq!(table.delimiter, ',');
        assert_eq!(table.rows.len(), 2);
    }

    #[test]
    fn miktar_import_uc_ondalik_bicimi_destekler() {
        assert_eq!(
            parse_quantity_import("1.234,56").unwrap(),
            Quantity::from_milli(1_234_560)
        );
        assert_eq!(
            parse_quantity_import("1234.56").unwrap(),
            Quantity::from_milli(1_234_560)
        );
        assert_eq!(
            parse_quantity_import("1234,56").unwrap(),
            Quantity::from_milli(1_234_560)
        );
        assert_eq!(
            parse_quantity_import("12,5 kg").unwrap(),
            Quantity::from_milli(12_500)
        );
        assert_eq!(parse_quantity_import("").unwrap(), Quantity::ZERO);
    }

    #[test]
    fn money_binlik_ayirici_yazmaz() {
        assert_eq!(money(Money::from_kurus(123_456)), "1234,56");
        assert_eq!(money(Money::from_kurus(-123_456)), "-1234,56");
    }
}
