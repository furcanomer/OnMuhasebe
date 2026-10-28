// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Kasa/banka hesabı iş modeli ve doğrulama kuralları.
//!
//! Bu modül **saf iş mantığıdır**: veritabanına ve arayüze bağımlılığı yoktur,
//! bu nedenle birim testleriyle eksiksiz doğrulanabilir.
//!
//! Üç ayrı tip vardır ve aralarındaki akış bilinçlidir:
//!
//! | Tip                    | Nerede kullanılır                                     |
//! |------------------------|-------------------------------------------------------|
//! | [`CashAccountDraft`]   | Arayüz formundaki ham (doğrulanmamış) kullanıcı girdisi |
//! | [`ValidatedCashAccount`] | Doğrulamadan geçmiş, veritabanına yazılabilir kayıt  |
//! | [`CashAccount`]        | Veritabanından okunmuş, kimliği olan kayıt            |
//!
//! Böylece geçersiz veri SQL katmanına **hiç ulaşmaz**: veritabanı yalnızca
//! [`ValidatedCashAccount`] kabul eder.
//!
//! ## Bakiye hakkında
//!
//! * `opening_balance` (açılış bakiyesi) hiçbir zaman negatif olamaz.
//! * `current_balance` (güncel bakiye) **işaretli** olabilir: kasa/banka
//!   hesabı eksiye düşebilir (bu durum sessizce kabul edilir).
//! * Kayıt **silinmez**: hesap `active` alanı ile pasife alınır.

use crate::money::Money;

/// Kasa/banka hesabı adı için azami uzunluk.
pub const NAME_MAX_LEN: usize = 100;

/// Banka adı için azami uzunluk.
pub const BANK_NAME_MAX_LEN: usize = 100;

/// Notlar için azami uzunluk.
pub const NOTES_MAX_LEN: usize = 1000;

/// Türk IBAN uzunluğu (`TR` + 24 rakam).
pub const IBAN_LEN: usize = 26;

/// Tür IBAN'ının ülke öneki.
pub const IBAN_PREFIX: &str = "TR";

/// Kasa/banka hesabının türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AccountType {
    /// Nakit kasa.
    #[default]
    Kasa,
    /// Banka hesabı.
    Banka,
}

impl AccountType {
    /// Seçim listesindeki tüm türler.
    pub const ALL: [Self; 2] = [Self::Kasa, Self::Banka];

    /// Arayüzde gösterilen Türkçe ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Kasa => "Kasa",
            Self::Banka => "Banka",
        }
    }

    /// Veritabanında saklanan değer.
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Kasa => "kasa",
            Self::Banka => "banka",
        }
    }

    /// Veritabanındaki değerden türü çözer.
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "kasa" => Some(Self::Kasa),
            "banka" => Some(Self::Banka),
            _ => None,
        }
    }
}

/// Veritabanından okunmuş kasa/banka hesabı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashAccount {
    /// Birincil anahtar.
    pub id: i64,
    /// Benzersiz hesap adı.
    pub name: String,
    /// Hesap türü.
    pub account_type: AccountType,
    /// Banka adı (yalnızca banka hesaplarında).
    pub bank_name: Option<String>,
    /// IBAN (yalnızca banka hesaplarında).
    pub iban: Option<String>,
    /// Açılış bakiyesi (kuruş, negatif olamaz).
    pub opening_balance: Money,
    /// Güncel bakiye (kuruş, işaretli olabilir).
    pub current_balance: Money,
    /// Notlar.
    pub notes: Option<String>,
    /// Kayıt aktif mi? (pasif kayıtlar silinmez, listede kalır)
    pub active: bool,
    /// Oluşturulma zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub created_at: String,
    /// Son güncellenme zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub updated_at: String,
    /// Bu hesaba bağlı tahsilat/ödeme kaydı sayısı (iptaller dâhil).
    pub payment_count: usize,
    /// Bu hesaba bağlı gelir/gider kaydı sayısı (iptaller dâhil).
    pub entry_count: usize,
}

impl CashAccount {
    /// Bu hesaba bağlı herhangi bir hareket var mı?
    ///
    /// Tahsilat/ödeme, gelir ve gider kayıtları "hareket" sayılır. İptal
    /// edilen kayıtlar da sayılır. Hareket varsa açılış bakiyesi **artık
    /// düzenlenemez**.
    pub fn has_movements(&self) -> bool {
        self.payment_count > 0 || self.entry_count > 0
    }

    /// Durum etiketi: `Aktif` / `Pasif`.
    pub fn status_label(&self) -> &'static str {
        if self.active { "Aktif" } else { "Pasif" }
    }

    /// Banka hesabı mı?
    pub fn is_bank(&self) -> bool {
        self.account_type == AccountType::Banka
    }

    /// Kısaltılmış IBAN gösterimi (`TR33 00…5678`); IBAN yoksa `None`.
    pub fn shortened_iban(&self) -> Option<String> {
        let iban = self.iban.as_deref()?;
        Some(shorten_iban(iban))
    }

    /// Arama ölçütüne uyuyor mu? (ad, banka adı ve IBAN)
    ///
    /// `needle` değeri [`crate::customer::normalize_for_search`] ile normalize
    /// edilmiş olmalıdır.
    pub fn matches_search(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let haystacks = [
            Some(self.name.as_str()),
            self.bank_name.as_deref(),
            self.iban.as_deref(),
        ];
        haystacks
            .into_iter()
            .flatten()
            .any(|value| crate::customer::normalize_for_search(value).contains(needle))
    }
}

/// IBAN'ı kısaltır: ilk 6 ve son 4 karakter gösterilir.
pub fn shorten_iban(iban: &str) -> String {
    let characters: Vec<char> = iban.chars().collect();
    if characters.len() <= 12 {
        return iban.to_owned();
    }
    let head: String = characters[..6].iter().collect();
    let tail: String = characters[characters.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// Kasa/banka listesi sorgu ölçütleri.
///
/// `account_type` ve `active` alanları `None` ise ilgili ölçüt uygulanmaz.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CashAccountFilter {
    /// Ad, banka adı veya IBAN üzerinde aranacak metin.
    pub search: String,
    /// Yalnızca bu türü göster (`None` = tümü).
    pub account_type: Option<AccountType>,
    /// Yalnızca bu durumu göster (`None` = tümü).
    pub active: Option<bool>,
}

impl CashAccountFilter {
    /// Ölçüt hiçbir şeyi kısıtlıyor mu?
    pub fn is_empty(&self) -> bool {
        self.search.trim().is_empty() && self.account_type.is_none() && self.active.is_none()
    }
}

/// Kasa/banka hesap toplamları (liste/özet başlığı için).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CashTotals {
    /// Kasa hesaplarının toplam güncel bakiyesi.
    pub cash: Money,
    /// Banka hesaplarının toplam güncel bakiyesi.
    pub bank: Money,
}

impl CashTotals {
    /// Genel toplam (kasa + banka).
    pub fn total(&self) -> Money {
        Money::from_kurus(self.cash.kurus().saturating_add(self.bank.kurus()))
    }

    /// Verilen hesapların güncel bakiyelerinden toplam hesaplar.
    pub fn from_accounts<'a, I>(accounts: I) -> Self
    where
        I: IntoIterator<Item = &'a CashAccount>,
    {
        let mut totals = Self::default();
        for account in accounts {
            match account.account_type {
                AccountType::Kasa => {
                    totals.cash = Money::from_kurus(
                        totals
                            .cash
                            .kurus()
                            .saturating_add(account.current_balance.kurus()),
                    );
                }
                AccountType::Banka => {
                    totals.bank = Money::from_kurus(
                        totals
                            .bank
                            .kurus()
                            .saturating_add(account.current_balance.kurus()),
                    );
                }
            }
        }
        totals
    }
}

/// Arayüz formundaki ham kullanıcı girdisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashAccountDraft {
    /// Hesap adı.
    pub name: String,
    /// Hesap türü.
    pub account_type: AccountType,
    /// Banka adı (banka hesabında).
    pub bank_name: String,
    /// IBAN (banka hesabında).
    pub iban: String,
    /// Açılış bakiyesi (serbest metin; `Money::parse` ile çözülür).
    pub opening_balance: String,
    /// Notlar.
    pub notes: String,
    /// Aktif/Pasif durumu.
    pub active: bool,
}

impl Default for CashAccountDraft {
    /// Yeni kayıt formunun başlangıç değerleri (yeni kayıt varsayılan olarak aktiftir).
    fn default() -> Self {
        Self {
            name: String::new(),
            account_type: AccountType::default(),
            bank_name: String::new(),
            iban: String::new(),
            opening_balance: String::new(),
            notes: String::new(),
            active: true,
        }
    }
}

impl CashAccountDraft {
    /// Var olan bir kaydı düzenlemek üzere form verisine dönüştürür.
    pub fn from_account(account: &CashAccount) -> Self {
        Self {
            name: account.name.clone(),
            account_type: account.account_type,
            bank_name: account.bank_name.clone().unwrap_or_default(),
            iban: account.iban.clone().unwrap_or_default(),
            opening_balance: account.opening_balance.format_turkish(),
            notes: account.notes.clone().unwrap_or_default(),
            active: account.active,
        }
    }

    /// Formdaki veriyi doğrular ve veritabanına yazılabilir kayda dönüştürür.
    ///
    /// Boş bırakılan isteğe bağlı alanlar `None` olur; metinler kırpılır.
    /// Birden fazla hata varsa **hepsi** birden döner.
    pub fn validate(&self) -> Result<ValidatedCashAccount, FieldErrors> {
        let mut errors = FieldErrors::default();

        let name = self.name.trim();
        if name.is_empty() {
            errors.push(Field::Name, "Hesap adı zorunludur.");
        } else if name.chars().count() > NAME_MAX_LEN {
            errors.push(
                Field::Name,
                format!("Hesap adı en fazla {NAME_MAX_LEN} karakter olabilir."),
            );
        }

        let (bank_name, iban) = if self.account_type == AccountType::Banka {
            let bank_name = limited_optional(
                &self.bank_name,
                BANK_NAME_MAX_LEN,
                Field::BankName,
                "Banka adı",
                &mut errors,
            );

            let iban = match validate_iban(&self.iban) {
                Ok(iban) => iban,
                Err(message) => {
                    errors.push(Field::Iban, message);
                    None
                }
            };

            (bank_name, iban)
        } else {
            // Kasa hesabında banka bilgileri tutulmaz.
            (None, None)
        };

        let opening_balance = if self.opening_balance.trim().is_empty() {
            Money::ZERO
        } else {
            match Money::parse(&self.opening_balance) {
                Ok(money) if money.is_negative() => {
                    errors.push(Field::OpeningBalance, "Açılış bakiyesi negatif olamaz.");
                    Money::ZERO
                }
                Ok(money) => money,
                Err(_) => {
                    errors.push(
                        Field::OpeningBalance,
                        "Açılış bakiyesi geçerli bir tutar olmalıdır (örnek: 1.234,56).",
                    );
                    Money::ZERO
                }
            }
        };

        let notes = limited_optional(
            &self.notes,
            NOTES_MAX_LEN,
            Field::Notes,
            "Notlar",
            &mut errors,
        );

        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(ValidatedCashAccount {
            name: name.to_owned(),
            account_type: self.account_type,
            bank_name,
            iban,
            opening_balance,
            notes,
            active: self.active,
        })
    }
}

/// Doğrulamadan geçmiş, veritabanına yazılabilir kasa/banka hesabı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCashAccount {
    /// Hesap adı (kırpılmış).
    pub name: String,
    /// Hesap türü.
    pub account_type: AccountType,
    /// Banka adı (yalnızca banka hesabında).
    pub bank_name: Option<String>,
    /// IBAN (yalnızca banka hesabında).
    pub iban: Option<String>,
    /// Açılış bakiyesi (kuruş).
    pub opening_balance: Money,
    /// Notlar.
    pub notes: Option<String>,
    /// Aktif/Pasif durumu.
    pub active: bool,
}

/// Formda hata üretilebilen alanlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// Hesap adı.
    Name,
    /// Banka adı.
    BankName,
    /// IBAN.
    Iban,
    /// Açılış bakiyesi.
    OpeningBalance,
    /// Notlar.
    Notes,
}

/// Alan bazlı doğrulama hataları.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldErrors {
    entries: Vec<(Field, String)>,
}

impl FieldErrors {
    /// Bir alana hata ekler.
    pub fn push(&mut self, field: Field, message: impl Into<String>) {
        self.entries.push((field, message.into()));
    }

    /// Alanın hata mesajı (yoksa `None`).
    pub fn get(&self, field: Field) -> Option<&str> {
        self.entries
            .iter()
            .find(|(entry, _)| *entry == field)
            .map(|(_, message)| message.as_str())
    }

    /// Alanın hatasını siler (kullanıcı alanı düzeltmeye başladığında).
    pub fn clear(&mut self, field: Field) {
        self.entries.retain(|(entry, _)| *entry != field);
    }

    /// Hiç hata yok mu?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Hata sayısı.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// IBAN biçimini doğrular.
///
/// Kurallar: boş bırakılabilir; doluysa boşluklar atılır, büyük harfe
/// çevrilir ve `TR` + 24 rakam (toplam 26 karakter) olmalıdır.
pub fn validate_iban(input: &str) -> Result<Option<String>, String> {
    let cleaned: String = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_uppercase();

    if cleaned.is_empty() {
        return Ok(None);
    }

    if cleaned.chars().count() != IBAN_LEN {
        return Err(format!(
            "IBAN {IBAN_LEN} karakter olmalıdır (örnek: TR33 0006 1005 1978 6457 8413 26)."
        ));
    }

    if !cleaned.starts_with(IBAN_PREFIX) {
        return Err(format!("IBAN '{IBAN_PREFIX}' ile başlamalıdır."));
    }

    if !cleaned[IBAN_PREFIX.len()..]
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        return Err("IBAN, ülke kodundan sonra yalnızca rakam içermelidir.".to_owned());
    }

    Ok(Some(cleaned))
}

/// Metni kırpar; boş kalırsa `None` döner.
fn trimmed_optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Uzunluk sınırlı isteğe bağlı alanı doğrular.
fn limited_optional(
    value: &str,
    max_len: usize,
    field: Field,
    label: &str,
    errors: &mut FieldErrors,
) -> Option<String> {
    let value = trimmed_optional(value);
    if let Some(value) = &value
        && value.chars().count() > max_len
    {
        errors.push(
            field,
            format!("{label} en fazla {max_len} karakter olabilir."),
        );
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_account() -> CashAccount {
        CashAccount {
            id: 1,
            name: "Merkez Kasa".to_owned(),
            account_type: AccountType::Kasa,
            bank_name: None,
            iban: None,
            opening_balance: Money::from_kurus(100_000),
            current_balance: Money::from_kurus(100_000),
            notes: None,
            active: true,
            created_at: "2026-10-18 10:00:00".to_owned(),
            updated_at: "2026-10-18 10:00:00".to_owned(),
            payment_count: 0,
            entry_count: 0,
        }
    }

    fn draft() -> CashAccountDraft {
        CashAccountDraft {
            name: "Merkez Kasa".to_owned(),
            ..CashAccountDraft::default()
        }
    }

    #[test]
    fn hesap_turu_etiketleri_ve_veritabani_karsiligi_tutarlidir() {
        for account_type in AccountType::ALL {
            assert!(!account_type.label().is_empty());
            assert_eq!(
                AccountType::from_db(account_type.as_db()),
                Some(account_type)
            );
        }
        assert_eq!(AccountType::from_db("bilinmeyen"), None);
    }

    #[test]
    fn gecerli_form_verisi_dogrulamadan_gecer() {
        let mut form = draft();
        form.opening_balance = " 1.234,56 ".to_owned();
        form.notes = "  Ana kasa  ".to_owned();

        let validated = form.validate().expect("doğrulama geçmeli");
        assert_eq!(validated.name, "Merkez Kasa");
        assert_eq!(validated.opening_balance, Money::from_kurus(123_456));
        assert_eq!(validated.notes.as_deref(), Some("Ana kasa"));
        assert!(validated.active);
        assert_eq!(validated.bank_name, None);
        assert_eq!(validated.iban, None);
    }

    #[test]
    fn bos_ad_ve_gecersiz_tutar_reddedilir() {
        let mut form = draft();
        form.name = "   ".to_owned();
        form.opening_balance = "-10".to_owned();

        let errors = form.validate().expect_err("doğrulama başarısız olmalı");
        assert!(errors.get(Field::Name).is_some());
        assert!(errors.get(Field::OpeningBalance).is_some());
    }

    #[test]
    fn uzun_ad_ve_not_reddedilir() {
        let mut form = draft();
        form.name = "A".repeat(NAME_MAX_LEN + 1);
        form.notes = "N".repeat(NOTES_MAX_LEN + 1);

        let errors = form.validate().expect_err("doğrulama başarısız olmalı");
        assert!(errors.get(Field::Name).is_some());
        assert!(errors.get(Field::Notes).is_some());
    }

    #[test]
    fn kasa_hesabinda_banka_bilgileri_yok_sayilir() {
        let mut form = draft();
        form.account_type = AccountType::Kasa;
        form.bank_name = "Ziraat".to_owned();
        form.iban = "TR330006100519786457841326".to_owned();

        let validated = form.validate().expect("doğrulama geçmeli");
        assert_eq!(validated.bank_name, None);
        assert_eq!(validated.iban, None);
    }

    #[test]
    fn banka_hesabinda_gecerli_iban_kabul_edilir() {
        let mut form = draft();
        form.account_type = AccountType::Banka;
        form.bank_name = "Ziraat Bankası".to_owned();
        form.iban = "tr33 0006 1005 1978 6457 8413 26".to_owned();

        let validated = form.validate().expect("doğrulama geçmeli");
        assert_eq!(validated.bank_name.as_deref(), Some("Ziraat Bankası"));
        assert_eq!(
            validated.iban.as_deref(),
            Some("TR330006100519786457841326")
        );
    }

    #[test]
    fn gecersiz_iban_reddedilir() {
        let mut form = draft();
        form.account_type = AccountType::Banka;
        form.iban = "TR12".to_owned();
        let errors = form.validate().expect_err("kısa IBAN reddedilmeli");
        assert!(errors.get(Field::Iban).is_some());

        form.iban = "DE330006100519786457841326".to_owned();
        let errors = form.validate().expect_err("yanlış ülke kodu reddedilmeli");
        assert!(errors.get(Field::Iban).is_some());

        form.iban = "TR33000610051978645784132X".to_owned();
        let errors = form
            .validate()
            .expect_err("rakam olmayan karakter reddedilmeli");
        assert!(errors.get(Field::Iban).is_some());
    }

    #[test]
    fn iban_bos_birakilabilir() {
        assert_eq!(validate_iban("").expect("boş IBAN geçerli"), None);
        assert_eq!(validate_iban("   ").expect("boşluk geçerli"), None);
    }

    #[test]
    fn iban_kisaltilir() {
        assert_eq!(shorten_iban("TR330006100519786457841326"), "TR3300…1326");
        assert_eq!(shorten_iban("TR1234"), "TR1234");
    }

    #[test]
    fn hareket_bilgisi_ve_durum_etiketleri_dogrudur() {
        let mut account = valid_account();
        assert!(!account.has_movements());
        assert_eq!(account.status_label(), "Aktif");
        assert!(!account.is_bank());

        account.payment_count = 2;
        assert!(account.has_movements());

        account.payment_count = 0;
        account.entry_count = 1;
        assert!(
            account.has_movements(),
            "gelir/gider kaydı da hareket sayılmalı"
        );

        account.active = false;
        assert_eq!(account.status_label(), "Pasif");
    }

    #[test]
    fn arama_turkce_karakterlerle_calisir() {
        let mut account = valid_account();
        account.name = "Öğle Kasa".to_owned();
        assert!(account.matches_search(&crate::customer::normalize_for_search("ogle")));
        assert!(!account.matches_search(&crate::customer::normalize_for_search("banka")));
    }

    #[test]
    fn toplamlar_tur_bazinda_hesaplanir() {
        let mut kasa = valid_account();
        kasa.current_balance = Money::from_kurus(50_000);
        let mut banka = valid_account();
        banka.id = 2;
        banka.account_type = AccountType::Banka;
        banka.current_balance = Money::from_kurus(-10_000);

        let totals = CashTotals::from_accounts([&kasa, &banka]);
        assert_eq!(totals.cash.kurus(), 50_000);
        assert_eq!(totals.bank.kurus(), -10_000);
        assert_eq!(totals.total().kurus(), 40_000);
    }
}
