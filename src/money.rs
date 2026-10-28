// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Para tipi.
//!
//! Tutarlar **hiçbir zaman** `f32`/`f64` ile saklanmaz. [`Money`] değeri
//! *kuruş* (1/100 TL) cinsinden `i64` olarak tutulur. Böylece:
//!
//! * Kayan noktalı yuvarlama hataları oluşmaz (`0.1 + 0.2 != 0.3` problemi yoktur).
//! * Tutar, SQLite'a doğrudan `INTEGER` kolonu olarak yazılabilir.
//! * Toplam/çıkarma işlemleri taşma kontrolü ile yapılabilir.
//!
//! Örnek: `12,34 TL` değeri bellekte `1234` kuruş olarak durur.

use std::fmt;
use std::str::FromStr;

use crate::error::{AppError, Result};
use crate::quantity::Quantity;

/// Kuruş cinsinden tam sayı tabanlı para tutarı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Money(i64);

impl Money {
    /// Sıfır tutar.
    pub const ZERO: Self = Self(0);

    /// Bir Türk Lirası kaç kuruştur.
    pub const KURUS_PER_LIRA: i64 = 100;

    /// Kuruş değerinden tutar oluşturur.
    pub const fn from_kurus(kurus: i64) -> Self {
        Self(kurus)
    }

    /// Tam lira değerinden tutar oluşturur.
    pub fn from_lira(lira: i64) -> Result<Self> {
        lira.checked_mul(Self::KURUS_PER_LIRA)
            .map(Self)
            .ok_or(AppError::AmountOverflow)
    }

    /// Tutarın kuruş değeri (veritabanında `INTEGER` olarak saklanır).
    pub const fn kurus(self) -> i64 {
        self.0
    }

    /// İki tutarı taşma kontrolü ile toplar.
    pub fn checked_add(self, other: Self) -> Result<Self> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(AppError::AmountOverflow)
    }

    /// İki tutarı taşma kontrolü ile çıkarır.
    pub fn checked_sub(self, other: Self) -> Result<Self> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(AppError::AmountOverflow)
    }

    /// Tutarı bir adet/miktar ile çarpar (ör. birim fiyat × miktar).
    pub fn checked_mul(self, factor: i64) -> Result<Self> {
        self.0
            .checked_mul(factor)
            .map(Self)
            .ok_or(AppError::AmountOverflow)
    }

    /// Birim fiyatı miktarla çarpar ve kuruşa yuvarlar (`birim fiyat × miktar`).
    ///
    /// Miktar mili-birim (1/1000) cinsinden olduğu için çarpım 1000'e bölünür.
    /// Ara hesap `i128` ile yapılır; yarım kuruş **yukarı** yuvarlanır. Sonuç
    /// `i64` aralığına sığmazsa [`AppError::AmountOverflow`] döner.
    pub fn checked_mul_quantity(self, quantity: Quantity) -> Result<Self> {
        let product = i128::from(self.0) * i128::from(quantity.milli());
        Self::from_ratio(product, i128::from(Quantity::MILLI_PER_UNIT))
    }

    /// Tutarı bir yüzde oranıyla çarpar ve kuruşa yuvarlar (iskonto/KDV).
    ///
    /// `percent` 0-100 arası bir orandır. Ara hesap `i128` ile yapılır; yarım
    /// kuruş **yukarı** yuvarlanır.
    pub fn checked_mul_percent(self, percent: i64) -> Result<Self> {
        let product = i128::from(self.0) * i128::from(percent);
        Self::from_ratio(product, 100)
    }

    /// Bu tutarın `base` tutarına oranını **tam sayı yüzde** (0-100) olarak verir.
    ///
    /// Oran, yarım yukarı yuvarlanarak hesaplanır ve `f32`/`f64` kullanılmaz.
    /// `base` sıfır veya negatifse oran tanımsızdır ve `None` döner. Böylece
    /// arayüz `%0` gibi yanıltıcı bir değer göstermez.
    pub fn percentage_of(self, base: Self) -> Option<i64> {
        if base.0 <= 0 {
            return None;
        }
        let numerator = i128::from(self.0.max(0)) * 100;
        let value = divide_round_half_up(numerator, i128::from(base.0));
        Some(value.clamp(0, 100) as i64)
    }

    /// `i128` ara sonucu, sıfırdan uzağa yarım yukarı yuvarlayarak kuruşa çevirir.
    fn from_ratio(numerator: i128, denominator: i128) -> Result<Self> {
        i64::try_from(divide_round_half_up(numerator, denominator))
            .map(Self)
            .map_err(|_| AppError::AmountOverflow)
    }

    /// Tutarın işaretini ters çevirir (borç/alacak yönü).
    pub fn checked_neg(self) -> Result<Self> {
        self.0
            .checked_neg()
            .map(Self)
            .ok_or(AppError::AmountOverflow)
    }

    /// Bir tutar dizisinin toplamını taşma kontrolü ile hesaplar.
    pub fn checked_sum<I>(amounts: I) -> Result<Self>
    where
        I: IntoIterator<Item = Self>,
    {
        amounts.into_iter().try_fold(Self::ZERO, Self::checked_add)
    }

    /// Tutar sıfır mı?
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Tutar negatif mi?
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Kuruş hanesi (0..=99).
    pub const fn kurus_part(self) -> u8 {
        (self.0.unsigned_abs() % Self::KURUS_PER_LIRA as u64) as u8
    }

    /// Tam lira hanesi (işaret korunur).
    pub const fn lira_part(self) -> i64 {
        self.0 / Self::KURUS_PER_LIRA
    }

    /// Türkçe biçimde yazar: binlik ayırıcı `.`, ondalık ayırıcı `,`.
    ///
    /// `1234567` kuruş -> `"12.345,67"`
    pub fn format_turkish(self) -> String {
        let negative = self.0 < 0;
        let absolute = self.0.unsigned_abs();
        let lira = absolute / Self::KURUS_PER_LIRA as u64;
        let kurus = absolute % Self::KURUS_PER_LIRA as u64;

        let digits = lira.to_string();
        let mut grouped = String::with_capacity(digits.len() + digits.len() / 3 + 4);
        for (index, character) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index).is_multiple_of(3) {
                grouped.push('.');
            }
            grouped.push(character);
        }

        let sign = if negative { "-" } else { "" };
        format!("{sign}{grouped},{kurus:02}")
    }

    /// Para sembolü eklenmiş gösterim: `"12.345,67 ₺"`.
    pub fn format_with_symbol(self) -> String {
        format!("{} ₺", self.format_turkish())
    }

    /// Metinden tutar çözümler.
    ///
    /// Ayrıştırma kuralları (boşluk, kesme işareti ve para sembolleri yok sayılır):
    ///
    /// * Hem `.` hem `,` varsa **en sonda gelen** ondalık ayırıcıdır
    ///   (`"1.234,56"` ve `"1,234.56"` aynı sonucu verir).
    /// * Yalnızca `,` varsa Türkçe kural geçerlidir: `,` **her zaman** ondalık
    ///   ayırıcıdır (`"1,5"` -> 1,50 TL).
    /// * Yalnızca `.` varsa ve ardından 1-2 hane geliyorsa ondalık ayırıcıdır
    ///   (`"1234.56"` -> 1234,56 TL); aksi halde binlik ayırıcıdır
    ///   (`"1.234"` -> 1234,00 TL).
    ///
    /// İkiden fazla ondalık hane içeren girdiler hata verir; çünkü kuruş
    /// hassasiyetinde tam olarak gösterilemezler ve sessizce yuvarlanmaları
    /// muhasebe açısından kabul edilemez.
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        let (sign, body) = match trimmed.strip_prefix('-') {
            Some(rest) => (-1_i64, rest),
            None => (1_i64, trimmed.strip_prefix('+').unwrap_or(trimmed)),
        };

        // Yalnızca rakam ve ayırıcıları tut; boşluk, sembol ve harfleri yok say.
        let mut cleaned = String::with_capacity(body.len());
        for character in body.chars() {
            match character {
                '0'..='9' | '.' | ',' => cleaned.push(character),
                ' ' | '\u{00A0}' | '\u{202F}' | '\'' | '_' => {}
                _ if is_currency_noise(character) => {}
                _ => return Err(AppError::InvalidAmount(input.to_owned())),
            }
        }

        if cleaned.is_empty() {
            return Err(AppError::InvalidAmount(input.to_owned()));
        }

        let decimal_index = decimal_separator_index(&cleaned);

        // Ondalık ayırıcı, karşıt binlik ayırıcı yokken yalnızca bir kez geçebilir.
        if let Some(index) = decimal_index {
            let separator = &cleaned[index..=index];
            let other = if separator == "." { ',' } else { '.' };
            if cleaned.matches(separator).count() > 1 && !cleaned.contains(other) {
                return Err(AppError::InvalidAmount(input.to_owned()));
            }
        }

        let (integer_text, fraction_text) = match decimal_index {
            Some(index) => (&cleaned[..index], &cleaned[index + 1..]),
            None => (cleaned.as_str(), ""),
        };

        validate_integer_part(integer_text, input)?;

        // Ondalık kısım yalnızca rakam olabilir ve en fazla iki hane içerebilir.
        if !fraction_text.chars().all(|c| c.is_ascii_digit()) || fraction_text.len() > 2 {
            return Err(AppError::InvalidAmount(input.to_owned()));
        }

        let integer_digits: String = integer_text.chars().filter(char::is_ascii_digit).collect();

        if integer_digits.is_empty() && fraction_text.is_empty() {
            return Err(AppError::InvalidAmount(input.to_owned()));
        }

        let lira = parse_digits(&integer_digits, input)?;

        let mut fraction_digits = fraction_text.to_owned();
        while fraction_digits.len() < 2 {
            fraction_digits.push('0');
        }
        let kurus = if fraction_digits.is_empty() {
            0
        } else {
            parse_digits(&fraction_digits, input)?
        };

        let magnitude = i128::from(lira) * i128::from(Self::KURUS_PER_LIRA) + i128::from(kurus);
        let signed = if sign < 0 { -magnitude } else { magnitude };
        let total = i64::try_from(signed).map_err(|_| AppError::AmountOverflow)?;

        Ok(Self(total))
    }
}

/// Ondalık ayırıcının bayt indeksini bulur; yoksa `None` döner.
fn decimal_separator_index(cleaned: &str) -> Option<usize> {
    match (cleaned.rfind('.'), cleaned.rfind(',')) {
        // Her iki ayırıcı da varsa en sonda gelen ondalık ayırıcıdır.
        (Some(dot), Some(comma)) => Some(dot.max(comma)),
        // Türkçe biçimde virgül her zaman ondalık ayırıcıdır.
        (None, Some(comma)) => Some(comma),
        // Nokta tek başına yalnızca ardından 1-2 hane geliyorsa ondalıktır.
        (Some(dot), None) => {
            let after = &cleaned[dot + 1..];
            let only_digits = after.chars().all(|c| c.is_ascii_digit());
            let digit_count = after.chars().filter(char::is_ascii_digit).count();
            (only_digits && (1..=2).contains(&digit_count)).then_some(dot)
        }
        (None, None) => None,
    }
}

/// Tam (lira) kısmındaki binlik ayırıcı kullanımını doğrular ve rakamları ayırır.
///
/// Boş tam kısım (`" ,50"`) geçerlidir. Ayırıcı karakteri tektir ve ilk grup
/// dışındaki tüm gruplar tam olarak 3 haneli olmalıdır.
fn validate_integer_part(integer_text: &str, original: &str) -> Result<()> {
    if integer_text.is_empty() {
        return Ok(());
    }

    if !integer_text
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    {
        return Err(AppError::InvalidAmount(original.to_owned()));
    }

    let separators: Vec<char> = integer_text
        .chars()
        .filter(|c| !c.is_ascii_digit())
        .collect();

    let Some(separator) = separators.first().copied() else {
        return Ok(());
    };

    if separators.iter().any(|c| *c != separator) {
        return Err(AppError::InvalidAmount(original.to_owned()));
    }

    for (index, group) in integer_text.split(separator).enumerate() {
        let length = group.chars().count();
        let valid = if index == 0 {
            (1..=3).contains(&length)
        } else {
            length == 3
        };
        if !valid {
            return Err(AppError::InvalidAmount(original.to_owned()));
        }
    }

    Ok(())
}

/// Yalnızca rakamlardan oluşan bir grubu sayıya çevirir.
fn parse_digits(digits: &str, original: &str) -> Result<i64> {
    if digits.is_empty() {
        return Ok(0);
    }
    digits
        .parse::<i64>()
        .map_err(|_| AppError::InvalidAmount(original.to_owned()))
}

/// Tutar metninde yok sayılabilecek para birimi işaretleri.
fn is_currency_noise(character: char) -> bool {
    matches!(character, '₺' | 'T' | 'L' | 't' | 'l' | '$' | '€' | '%')
}

/// Payı paydaya böler ve **yarım kuruşu sıfırdan uzağa** yuvarlar.
///
/// Yalnızca `i128` ara sonuçlarda kullanılır; `denominator` pozitif olmalıdır.
fn divide_round_half_up(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        -((-numerator + denominator / 2) / denominator)
    }
}

impl fmt::Display for Money {
    /// `"12.345,67 ₺"` biçiminde yazar.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.format_with_symbol())
    }
}

impl FromStr for Money {
    type Err = AppError;

    fn from_str(text: &str) -> Result<Self> {
        Self::parse(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kurus_ve_lira_donusumu() {
        assert_eq!(Money::from_lira(12).unwrap().kurus(), 1200);
        assert_eq!(Money::from_kurus(1234).lira_part(), 12);
        assert_eq!(Money::from_kurus(1234).kurus_part(), 34);
        assert_eq!(Money::ZERO.kurus(), 0);
    }

    #[test]
    fn negatif_tutarlarin_hane_ayrimi() {
        let money = Money::from_kurus(-1234);
        assert_eq!(money.lira_part(), -12);
        assert_eq!(money.kurus_part(), 34);
        assert!(money.is_negative());
    }

    #[test]
    fn turkce_bicimlendirme_binlik_ayrac_kullanir() {
        assert_eq!(Money::from_kurus(0).format_turkish(), "0,00");
        assert_eq!(Money::from_kurus(5).format_turkish(), "0,05");
        assert_eq!(Money::from_kurus(123_456).format_turkish(), "1.234,56");
        assert_eq!(
            Money::from_kurus(123_456_789).format_turkish(),
            "1.234.567,89"
        );
        assert_eq!(Money::from_kurus(-123_456).format_turkish(), "-1.234,56");
        assert_eq!(
            Money::from_kurus(123_456).format_with_symbol(),
            "1.234,56 ₺"
        );
    }

    #[test]
    fn tutar_cozumleme_turkce_bicim() {
        assert_eq!(Money::parse("1.234,56").unwrap().kurus(), 123_456);
        assert_eq!(Money::parse("1234,56").unwrap().kurus(), 123_456);
        assert_eq!(Money::parse("0,05").unwrap().kurus(), 5);
        assert_eq!(Money::parse("42").unwrap().kurus(), 4200);
        assert_eq!(Money::parse("1.234").unwrap().kurus(), 123_400);
        assert_eq!(Money::parse("-42").unwrap().kurus(), -4200);
        assert_eq!(Money::parse("1,5").unwrap().kurus(), 150);
        assert_eq!(Money::parse("1.234,56 ₺").unwrap().kurus(), 123_456);
    }

    #[test]
    fn tutar_cozumleme_ingilizce_bicim() {
        assert_eq!(Money::parse("1234.56").unwrap().kurus(), 123_456);
        assert_eq!(Money::parse("1,234.56").unwrap().kurus(), 123_456);
        assert_eq!(Money::parse("1234.5").unwrap().kurus(), 123_450);
    }

    #[test]
    fn ondalik_hanesi_olmayan_girdiler() {
        assert_eq!(Money::parse(",50").unwrap().kurus(), 50);
        assert_eq!(Money::parse("1.234.567").unwrap().kurus(), 123_456_700);
    }

    #[test]
    fn gecersiz_tutarlar_hata_verir() {
        for input in [
            "",
            "   ",
            "abc",
            "1,2,3",
            "1,234,567", // ABD biçimli binlik ayırıcı desteklenmez
            "12,345",
            "1.234,567",
            "1.23.4",
            "--5",
        ] {
            assert!(
                Money::parse(input).is_err(),
                "'{input}' geçersiz sayılmalıydı"
            );
        }
    }

    #[test]
    fn toplama_ve_cikarma_tasma_kontrolu_yapar() {
        let a = Money::from_kurus(100);
        let b = Money::from_kurus(250);
        assert_eq!(a.checked_add(b).unwrap().kurus(), 350);
        assert_eq!(b.checked_sub(a).unwrap().kurus(), 150);
        assert!(
            Money::from_kurus(i64::MAX)
                .checked_add(Money::from_kurus(1))
                .is_err()
        );
        assert!(Money::from_kurus(i64::MIN).checked_neg().is_err());
    }

    #[test]
    fn toplu_tutar_hesabi() {
        let amounts = [
            Money::from_kurus(100),
            Money::from_kurus(200),
            Money::from_kurus(300),
        ];
        assert_eq!(Money::checked_sum(amounts).unwrap().kurus(), 600);
        assert_eq!(Money::checked_sum([]).unwrap(), Money::ZERO);
    }

    #[test]
    fn birim_fiyat_miktar_carpimi_yarim_kurusu_yukari_yuvarlar() {
        // 1,50 ₺ × 2,5 = 3,75 ₺ (tam).
        assert_eq!(
            Money::from_kurus(150)
                .checked_mul_quantity(Quantity::from_milli(2500))
                .unwrap()
                .kurus(),
            375
        );

        // 1 kuruş × 1,5 = 1,5 kuruş -> 2 kuruş (yarım yukarı).
        assert_eq!(
            Money::from_kurus(1)
                .checked_mul_quantity(Quantity::from_milli(1500))
                .unwrap()
                .kurus(),
            2
        );

        // 1,00 ₺ × 0,005 = 0,5 kuruş -> 1 kuruş.
        assert_eq!(
            Money::from_kurus(100)
                .checked_mul_quantity(Quantity::from_milli(5))
                .unwrap()
                .kurus(),
            1
        );

        // Sıfır miktar sıfır tutar verir.
        assert_eq!(
            Money::from_kurus(999)
                .checked_mul_quantity(Quantity::ZERO)
                .unwrap()
                .kurus(),
            0
        );
    }

    #[test]
    fn birim_fiyat_miktar_carpimi_tasmayi_yakalar() {
        assert!(
            Money::from_kurus(i64::MAX)
                .checked_mul_quantity(Quantity::from_milli(2000))
                .is_err(),
            "i64 sınırını aşan çarpım hata vermeli"
        );
    }

    #[test]
    fn yuzde_carpimi_yarim_kurusu_yukari_yuvarlar() {
        assert_eq!(
            Money::from_kurus(1000)
                .checked_mul_percent(20)
                .unwrap()
                .kurus(),
            200
        );
        assert_eq!(
            Money::from_kurus(1000)
                .checked_mul_percent(18)
                .unwrap()
                .kurus(),
            180
        );
        assert_eq!(
            Money::from_kurus(1000)
                .checked_mul_percent(0)
                .unwrap()
                .kurus(),
            0
        );
        // 2,50 ₺ × %1 = 0,025 ₺ = 2,5 kuruş -> 3 kuruş.
        assert_eq!(
            Money::from_kurus(250)
                .checked_mul_percent(1)
                .unwrap()
                .kurus(),
            3
        );
        // 1,00 ₺ × %50 = 0,5 ₺ (tam).
        assert_eq!(
            Money::from_kurus(100)
                .checked_mul_percent(50)
                .unwrap()
                .kurus(),
            50
        );
    }

    #[test]
    fn yuzde_orani_tam_sayi_ve_yarim_yukari_hesaplanir() {
        let base = Money::from_kurus(1000);
        assert_eq!(Money::from_kurus(250).percentage_of(base), Some(25));
        assert_eq!(
            Money::from_kurus(333).percentage_of(base),
            Some(33),
            "33,3 -> 33"
        );
        assert_eq!(
            Money::from_kurus(335).percentage_of(base),
            Some(34),
            "33,5 -> 34 (yarım yukarı)"
        );
        assert_eq!(Money::ZERO.percentage_of(base), Some(0));
        assert_eq!(base.percentage_of(base), Some(100));
        // Sıfır taban oranı tanımsız yapar; arayüz "%0" göstermez.
        assert_eq!(Money::from_kurus(100).percentage_of(Money::ZERO), None);
        assert_eq!(
            Money::from_kurus(100).percentage_of(Money::from_kurus(-5)),
            None
        );
    }

    #[test]
    fn display_ve_from_str_tutarli_calisir() {
        let money: Money = "1.234,56".parse().unwrap();
        assert_eq!(money.to_string(), "1.234,56 ₺");
    }
}
