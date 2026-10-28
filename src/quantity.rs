// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Miktar tipi (stok adetleri için).
//!
//! Miktarlar **hiçbir zaman** `f32`/`f64` ile saklanmaz. [`Quantity`] değeri
//! *mili-birim* (bir birimin binde biri) cinsinden `i64` olarak tutulur.
//! Böylece:
//!
//! * Ondalıklı miktarlar (örneğin `1,5 kg`) tam sayı ile hatasız saklanır.
//! * Miktar, SQLite'a doğrudan `INTEGER` kolonu olarak yazılabilir.
//! * Toplama/çıkarma işlemleri taşma kontrolü ile yapılabilir.
//!
//! Örnek: `1,5 kg` değeri bellekte `1500` mili-birim olarak durur; `1500 kg`
//! ise `1_500_000` mili-birimdir.
//!
//! ## Ayrıştırma kuralı
//!
//! Türkçe biçim esastır: **virgül** ondalık ayırıcı, **nokta** binlik
//! ayırıcıdır (`1.500,25`). Böylece biçimlendirme ile ayrıştırma arasında
//! belirsizlik oluşmaz (`Quantity::format_turkish` çıktısı her zaman geri
//! okunabilir).

use std::fmt;
use std::str::FromStr;

use crate::error::{AppError, Result};

/// Mili-birim (binde bir) cinsinden tam sayı tabanlı miktar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Quantity(i64);

impl Quantity {
    /// Sıfır miktar.
    pub const ZERO: Self = Self(0);

    /// Bir birim kaç mili-birimdir.
    pub const MILLI_PER_UNIT: i64 = 1000;

    /// Ondalık kısmın en fazla hane sayısı.
    pub const FRACTION_MAX_LEN: usize = 3;

    /// Mili-birim değerinden miktar oluşturur.
    pub const fn from_milli(milli: i64) -> Self {
        Self(milli)
    }

    /// Tam birim değerinden miktar oluşturur.
    pub fn from_units(units: i64) -> Result<Self> {
        units
            .checked_mul(Self::MILLI_PER_UNIT)
            .map(Self)
            .ok_or(AppError::QuantityOverflow)
    }

    /// Miktarın mili-birim değeri (veritabanında `INTEGER` olarak saklanır).
    pub const fn milli(self) -> i64 {
        self.0
    }

    /// İki miktarı taşma kontrolü ile toplar.
    pub fn checked_add(self, other: Self) -> Result<Self> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(AppError::QuantityOverflow)
    }

    /// İki miktarı taşma kontrolü ile çıkarır.
    pub fn checked_sub(self, other: Self) -> Result<Self> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(AppError::QuantityOverflow)
    }

    /// Miktarın işaretini ters çevirir (stok giriş/çıkış yönü).
    pub fn checked_neg(self) -> Result<Self> {
        self.0
            .checked_neg()
            .map(Self)
            .ok_or(AppError::QuantityOverflow)
    }

    /// Miktar sıfır mı?
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Miktar negatif mi?
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Miktar pozitif mi?
    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Türkçe biçimde yazar: binlik ayırıcı `.`, ondalık ayırıcı `,`.
    ///
    /// Ondalık kısımdaki gereksiz sıfırlar atılır:
    ///
    /// * `1500` mili -> `"1,5"`
    /// * `1000` mili -> `"1"`
    /// * `1` mili -> `"0,001"`
    /// * `1_500_000` mili -> `"1.500"`
    pub fn format_turkish(self) -> String {
        let negative = self.0 < 0;
        let absolute = self.0.unsigned_abs();
        let units = absolute / Self::MILLI_PER_UNIT as u64;
        let milli = (absolute % Self::MILLI_PER_UNIT as u64) as u32;

        let digits = units.to_string();
        let mut grouped = String::with_capacity(digits.len() + digits.len() / 3 + 5);
        for (index, character) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index).is_multiple_of(3) {
                grouped.push('.');
            }
            grouped.push(character);
        }

        let sign = if negative { "-" } else { "" };

        if milli == 0 {
            format!("{sign}{grouped}")
        } else {
            let mut fraction = format!("{milli:03}");
            while fraction.ends_with('0') {
                fraction.pop();
            }
            format!("{sign}{grouped},{fraction}")
        }
    }

    /// İşaretini her zaman göstererek yazar (`+2,5`, `-3`, `0`).
    ///
    /// Stok hareketlerinin yönünü vurgulamak için kullanılır.
    pub fn format_signed_turkish(self) -> String {
        if self.0 > 0 {
            format!("+{}", self.format_turkish())
        } else {
            self.format_turkish()
        }
    }

    /// Miktarı birim etiketiyle birlikte biçimlendirir (`"1,5 kg"`).
    pub fn format_with_label(self, label: &str) -> String {
        format!("{} {label}", self.format_turkish())
    }

    /// İşaretli biçimde, birim etiketiyle birlikte (`"+2,5 kg"`).
    pub fn format_signed_with_label(self, label: &str) -> String {
        format!("{} {label}", self.format_signed_turkish())
    }

    /// Metinden miktar çözümler.
    ///
    /// Virgül ondalık (en fazla 3 hane), nokta binlik ayırıcıdır. Metin
    /// içindeki boşluklar ve birim etiketleri (örneğin `kg`) yok sayılır.
    /// İşaret kabul edilir; negatiflik çağıran katman tarafından değerlendirilir.
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
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
                _ => return Err(AppError::InvalidQuantity(input.to_owned())),
            }
        }

        if cleaned.is_empty() {
            return Err(AppError::InvalidQuantity(input.to_owned()));
        }

        let (decimal_index, thousands) = separators(&cleaned);
        let (integer_text, fraction_text) = match decimal_index {
            Some(index) => (&cleaned[..index], &cleaned[index + 1..]),
            None => (cleaned.as_str(), ""),
        };

        let integer_digits = integer_digits(integer_text, thousands, input)?;

        if !fraction_text.chars().all(|c| c.is_ascii_digit())
            || fraction_text.len() > Self::FRACTION_MAX_LEN
        {
            return Err(AppError::InvalidQuantity(input.to_owned()));
        }

        if integer_digits.is_empty() && fraction_text.is_empty() {
            return Err(AppError::InvalidQuantity(input.to_owned()));
        }

        let units = parse_digits(&integer_digits, input)?;

        let milli = if fraction_text.is_empty() {
            0
        } else {
            let mut fraction = fraction_text.to_owned();
            while fraction.len() < Self::FRACTION_MAX_LEN {
                fraction.push('0');
            }
            parse_digits(&fraction, input)?
        };

        let magnitude = i128::from(units) * i128::from(Self::MILLI_PER_UNIT) + i128::from(milli);
        let signed = if sign < 0 { -magnitude } else { magnitude };
        let total = i64::try_from(signed).map_err(|_| AppError::QuantityOverflow)?;

        Ok(Self(total))
    }
}

/// Ondalık ayırıcının bayt indeksini ve (varsa) binlik ayırıcıyı bulur.
fn separators(cleaned: &str) -> (Option<usize>, Option<char>) {
    match (cleaned.rfind('.'), cleaned.rfind(',')) {
        // İki ayırıcı da varsa en sonda gelen ondalıktır, diğeri binliktir.
        (Some(dot), Some(comma)) => {
            if dot > comma {
                (Some(dot), Some(','))
            } else {
                (Some(comma), Some('.'))
            }
        }
        // Yalnızca virgül: Türkçe biçimde her zaman ondalık ayırıcıdır.
        (None, Some(comma)) => (Some(comma), None),
        // Yalnızca nokta: binlik ayırıcıdır (ondalık için virgül kullanılır).
        (Some(_), None) => (None, Some('.')),
        (None, None) => (None, None),
    }
}

/// Tam kısımdaki binlik ayırıcı kullanımını doğrular ve yalnızca rakamları döndürür.
fn integer_digits(text: &str, thousands: Option<char>, original: &str) -> Result<String> {
    if text.is_empty() {
        return Ok(String::new());
    }

    let allowed = |character: char| character.is_ascii_digit() || Some(character) == thousands;
    if !text.chars().all(allowed) {
        return Err(AppError::InvalidQuantity(original.to_owned()));
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
                return Err(AppError::InvalidQuantity(original.to_owned()));
            }
        }
    }

    Ok(text.chars().filter(char::is_ascii_digit).collect())
}

/// Yalnızca rakamlardan oluşan bir grubu sayıya çevirir.
fn parse_digits(digits: &str, original: &str) -> Result<i64> {
    if digits.is_empty() {
        return Ok(0);
    }
    digits
        .parse::<i64>()
        .map_err(|_| AppError::InvalidQuantity(original.to_owned()))
}

impl fmt::Display for Quantity {
    /// `"1,5"` biçiminde yazar.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.format_turkish())
    }
}

impl FromStr for Quantity {
    type Err = AppError;

    fn from_str(text: &str) -> Result<Self> {
        Self::parse(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birim_ve_mili_donusumu() {
        assert_eq!(Quantity::from_units(3).unwrap().milli(), 3000);
        assert_eq!(Quantity::from_milli(1500).format_turkish(), "1,5");
        assert_eq!(Quantity::ZERO.milli(), 0);
        assert!(Quantity::ZERO.is_zero());
        assert!(!Quantity::ZERO.is_positive());
    }

    #[test]
    fn turkce_bicimlendirme_ondalik_ve_binlik_kullanir() {
        assert_eq!(Quantity::from_milli(0).format_turkish(), "0");
        assert_eq!(Quantity::from_milli(1).format_turkish(), "0,001");
        assert_eq!(Quantity::from_milli(500).format_turkish(), "0,5");
        assert_eq!(Quantity::from_milli(1500).format_turkish(), "1,5");
        assert_eq!(Quantity::from_milli(1000).format_turkish(), "1");
        assert_eq!(Quantity::from_milli(1234).format_turkish(), "1,234");
        assert_eq!(Quantity::from_milli(1_500_000).format_turkish(), "1.500");
        assert_eq!(Quantity::from_milli(1_234_500).format_turkish(), "1.234,5");
        assert_eq!(Quantity::from_milli(-1500).format_turkish(), "-1,5");
    }

    #[test]
    fn isaretli_ve_birimli_bicim() {
        assert_eq!(Quantity::from_milli(2500).format_signed_turkish(), "+2,5");
        assert_eq!(Quantity::from_milli(-3000).format_signed_turkish(), "-3");
        assert_eq!(Quantity::from_milli(0).format_signed_turkish(), "0");
        assert_eq!(Quantity::from_milli(1500).format_with_label("kg"), "1,5 kg");
        assert_eq!(
            Quantity::from_milli(-1500).format_signed_with_label("adet"),
            "-1,5 adet"
        );
    }

    #[test]
    fn miktar_cozumleme_turkce_bicim() {
        assert_eq!(Quantity::parse("1,5").unwrap().milli(), 1500);
        assert_eq!(Quantity::parse("1500").unwrap().milli(), 1_500_000);
        assert_eq!(Quantity::parse("1.500").unwrap().milli(), 1_500_000);
        assert_eq!(Quantity::parse("1.234,5").unwrap().milli(), 1_234_500);
        assert_eq!(Quantity::parse("0,001").unwrap().milli(), 1);
        assert_eq!(Quantity::parse(",5").unwrap().milli(), 500);
        assert_eq!(Quantity::parse("-2").unwrap().milli(), -2000);
        assert_eq!(Quantity::parse(" 3 ").unwrap().milli(), 3000);
        assert_eq!(Quantity::parse("2 kg").unwrap().milli(), 2000);
    }

    #[test]
    fn bicimlendirme_ve_ayristirma_tur_tutar() {
        for value in [0_i64, 1, 5, 999, 1000, 1500, 1_234_567, -2500] {
            let quantity = Quantity::from_milli(value);
            let text = quantity.format_turkish();
            assert_eq!(
                Quantity::parse(&text).unwrap(),
                quantity,
                "'{text}' geri okunabilmeli"
            );
        }
    }

    #[test]
    fn gecersiz_miktarlar_hata_verir() {
        for input in [
            "", "   ", "abc", "1,2,3", "1,2345", "1.23", "1.2", "--5", ".",
        ] {
            assert!(
                Quantity::parse(input).is_err(),
                "'{input}' geçersiz miktar sayılmalıydı"
            );
        }
    }

    #[test]
    fn toplama_cikarma_ve_tasma_kontrolu() {
        let a = Quantity::from_milli(1500);
        let b = Quantity::from_milli(2500);
        assert_eq!(a.checked_add(b).unwrap().milli(), 4000);
        assert_eq!(b.checked_sub(a).unwrap().milli(), 1000);
        assert_eq!(a.checked_neg().unwrap().milli(), -1500);
        assert!(
            Quantity::from_milli(i64::MAX)
                .checked_add(Quantity::from_milli(1))
                .is_err()
        );
        assert!(Quantity::from_milli(i64::MIN).checked_neg().is_err());
        assert!(Quantity::from_units(i64::MAX).is_err());
    }

    #[test]
    fn display_ve_from_str_tutarli_calisir() {
        let quantity: Quantity = "1.234,5".parse().unwrap();
        assert_eq!(quantity.to_string(), "1.234,5");
    }
}
