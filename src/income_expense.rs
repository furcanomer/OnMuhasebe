// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Gelir/gider kategorileri ve hareketleri için iş modeli ve doğrulama.
//!
//! Bu modül **saf iş mantığıdır**: veritabanına ve arayüze bağımlılığı yoktur.
//!
//! ## Gelir/gider ile tahsilat/ödeme arasındaki fark
//!
//! * **Tahsilat/ödeme** bir cari hesaba bağlıdır; cari bakiyesini ve fatura
//!   ödenen tutarını etkiler.
//! * **Gelir/gider** cari hesaba **bağlı değildir** (kira, elektrik, maaş,
//!   kırtasiye, kira geliri, faiz geliri ...). Yalnızca kasa/banka bakiyesini
//!   etkiler; cari bakiyesine ve faturalara dokunmaz.
//!
//! ## Tipler ve akış
//!
//! | Tip                    | Nerede kullanılır                                      |
//! |------------------------|--------------------------------------------------------|
//! | [`CategoryDraft`]      | Kategori formundaki ham (doğrulanmamış) kullanıcı girdisi |
//! | [`ValidatedCategory`]  | Doğrulamadan geçmiş, veritabanına yazılabilir kategori  |
//! | [`ExpenseCategory`]    | Veritabanından okunmuş, kimliği olan kategori           |
//! | [`EntryDraft`]         | Hareket formundaki ham kullanıcı girdisi                |
//! | [`ValidatedEntry`]     | Doğrulamadan geçmiş, veritabanına yazılabilir hareket   |
//! | [`IncomeExpenseEntry`] | Veritabanından okunmuş, kimliği olan hareket            |
//!
//! ## Kurallar
//!
//! * Kategori adı **türü içinde** benzersizdir; karşılaştırma büyük/küçük
//!   harf duyarsızdır. Bu kural veritabanında `UNIQUE (name, category_type)`
//!   ile de korunur.
//! * Kategori **silinmez**, `active = 0` ile pasife alınır. Pasif kategori
//!   yeni hareketlerde seçilemez; eski kayıtlarda görünmeye devam eder.
//! * Tutar **sıfırdan büyüktür** ve kuruş cinsinden saklanır.
//! * Hareket **silinmez/düzenlenmez**: düzeltme, iptal + yeni kayıt ile
//!   yapılır. İptal, kasa/banka bakiyesine olan etkiyi geri alır.
//! * Kasa/banka bakiyesi eksiye düşebilir; bu durum sessizce kabul edilir.

use crate::customer::{normalize_for_search, trimmed_optional};
use crate::money::Money;

/// Kategori adı için üst sınır (karakter).
pub const CATEGORY_NAME_MAX_LEN: usize = 100;
/// Kategori açıklaması için üst sınır (karakter).
pub const CATEGORY_DESCRIPTION_MAX_LEN: usize = 500;
/// Hareket açıklaması için üst sınır (karakter).
pub const DESCRIPTION_MAX_LEN: usize = 500;
/// Belge/fiş numarası için üst sınır (karakter).
pub const DOCUMENT_REF_MAX_LEN: usize = 100;
/// İptal nedeni için üst sınır (karakter).
pub const CANCEL_REASON_MAX_LEN: usize = 500;

/// Gelir/gider türü. Kategori türü ve hareket türü için ortak kullanılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum EntryType {
    /// Gelir: kasa/banka bakiyesini artırır.
    Gelir,
    /// Gider: kasa/banka bakiyesini azaltır.
    #[default]
    Gider,
}

impl EntryType {
    /// Seçim listesindeki tüm türler.
    pub const ALL: [Self; 2] = [Self::Gelir, Self::Gider];

    /// Arayüzde gösterilen Türkçe ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Gelir => "Gelir",
            Self::Gider => "Gider",
        }
    }

    /// Yeni kayıt düğmesi metni.
    pub fn new_label(self) -> &'static str {
        match self {
            Self::Gelir => "Yeni Gelir",
            Self::Gider => "Yeni Gider",
        }
    }

    /// Kaydet düğmesi metni.
    pub fn save_label(self) -> &'static str {
        match self {
            Self::Gelir => "Geliri Kaydet",
            Self::Gider => "Gideri Kaydet",
        }
    }

    /// Veritabanında saklanan değer.
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Gelir => "gelir",
            Self::Gider => "gider",
        }
    }

    /// Veritabanındaki değerden türü çözer.
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "gelir" => Some(Self::Gelir),
            "gider" => Some(Self::Gider),
            _ => None,
        }
    }

    /// Kasa/banka bakiyesine etki yönü: gelir `+1`, gider `-1`.
    pub fn cash_sign(self) -> i64 {
        match self {
            Self::Gelir => 1,
            Self::Gider => -1,
        }
    }
}

/// Gelir/gider hareketinin durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EntryStatus {
    /// Kesinleşmiş kayıt (kasa/banka bakiyesine yansımıştır).
    #[default]
    Kesin,
    /// İptal edilmiş kayıt (kasa/banka etkisi geri alınmıştır).
    Iptal,
}

impl EntryStatus {
    /// Tüm durumlar (filtre listesi sırası).
    pub const ALL: [Self; 2] = [Self::Kesin, Self::Iptal];

    /// Arayüzde gösterilen Türkçe ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Kesin => "Kesin",
            Self::Iptal => "İptal",
        }
    }

    /// Veritabanında saklanan değer.
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Kesin => "kesin",
            Self::Iptal => "iptal",
        }
    }

    /// Veritabanındaki değerden durumu çözer.
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "kesin" => Some(Self::Kesin),
            "iptal" => Some(Self::Iptal),
            _ => None,
        }
    }
}

/// Veritabanından okunmuş gelir/gider kategorisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpenseCategory {
    /// Birincil anahtar.
    pub id: i64,
    /// Kategori adı.
    pub name: String,
    /// Kategori türü.
    pub category_type: EntryType,
    /// Açıklama.
    pub description: Option<String>,
    /// Aktif mi? Pasif kategoriler yeni hareketlerde seçilemez.
    pub active: bool,
    /// Bu kategoriyi kullanan hareket sayısı (iptaller dahil).
    pub usage_count: usize,
    /// Oluşturulma zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub created_at: String,
    /// Son güncellenme zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub updated_at: String,
}

impl ExpenseCategory {
    /// Kategori aktif mi?
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Durum etiketi: `Aktif` / `Pasif`.
    pub fn status_label(&self) -> &'static str {
        if self.active { "Aktif" } else { "Pasif" }
    }

    /// Arama ölçütüne uyuyor mu? (ad veya açıklama)
    ///
    /// `needle` değeri [`crate::customer::normalize_for_search`] ile normalize
    /// edilmiş olmalıdır.
    pub fn matches_search(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let haystacks = [Some(self.name.as_str()), self.description.as_deref()];
        haystacks
            .into_iter()
            .flatten()
            .any(|value| normalize_for_search(value).contains(needle))
    }
}

/// Kategori listesi sorgu ölçütleri.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CategoryFilter {
    /// Ad veya açıklamada aranacak metin.
    pub search: String,
    /// Yalnızca bu türü göster (`None` = tümü).
    pub category_type: Option<EntryType>,
    /// Yalnızca bu durumu göster (`None` = tümü).
    pub active: Option<bool>,
}

impl CategoryFilter {
    /// Ölçüt hiçbir şeyi kısıtlamıyor mu?
    pub fn is_empty(&self) -> bool {
        self.search.trim().is_empty() && self.category_type.is_none() && self.active.is_none()
    }
}

/// Kategori formundaki ham kullanıcı girdisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryDraft {
    /// Kategori adı.
    pub name: String,
    /// Kategori türü.
    pub category_type: EntryType,
    /// Açıklama.
    pub description: String,
    /// Aktif mi?
    pub active: bool,
}

impl Default for CategoryDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            category_type: EntryType::Gider,
            description: String::new(),
            active: true,
        }
    }
}

impl CategoryDraft {
    /// Var olan kategoriden form değerleri üretir (düzenleme).
    pub fn from_category(category: &ExpenseCategory) -> Self {
        Self {
            name: category.name.clone(),
            category_type: category.category_type,
            description: category.description.clone().unwrap_or_default(),
            active: category.active,
        }
    }

    /// Formdaki veriyi doğrular.
    pub fn validate(&self) -> Result<ValidatedCategory, CategoryFieldErrors> {
        let mut errors = CategoryFieldErrors::default();

        let name = self.name.trim();
        if name.is_empty() {
            errors.push(CategoryField::Name, "Kategori adı zorunludur.");
        } else if name.chars().count() > CATEGORY_NAME_MAX_LEN {
            errors.push(
                CategoryField::Name,
                format!("Kategori adı en fazla {CATEGORY_NAME_MAX_LEN} karakter olabilir."),
            );
        }

        let description = trimmed_optional(&self.description);
        if let Some(text) = &description
            && text.chars().count() > CATEGORY_DESCRIPTION_MAX_LEN
        {
            errors.push(
                CategoryField::Description,
                format!("Açıklama en fazla {CATEGORY_DESCRIPTION_MAX_LEN} karakter olabilir."),
            );
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(ValidatedCategory {
            name: name.to_owned(),
            category_type: self.category_type,
            description,
            active: self.active,
        })
    }
}

/// Doğrulamadan geçmiş, veritabanına yazılabilir kategori.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCategory {
    /// Kategori adı (kırpılmış).
    pub name: String,
    /// Kategori türü.
    pub category_type: EntryType,
    /// Açıklama.
    pub description: Option<String>,
    /// Aktif mi?
    pub active: bool,
}

/// Kategori formunda hata üretilebilen alanlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CategoryField {
    /// Kategori adı.
    Name,
    /// Açıklama.
    Description,
}

/// Alan bazlı kategori doğrulama hataları.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CategoryFieldErrors {
    entries: Vec<(CategoryField, String)>,
}

impl CategoryFieldErrors {
    /// Bir alana hata ekler.
    pub fn push(&mut self, field: CategoryField, message: impl Into<String>) {
        self.entries.push((field, message.into()));
    }

    /// Alanın hata mesajı (yoksa `None`).
    pub fn get(&self, field: CategoryField) -> Option<&str> {
        self.entries
            .iter()
            .find(|(entry, _)| *entry == field)
            .map(|(_, message)| message.as_str())
    }

    /// Alanın hatasını siler.
    pub fn clear(&mut self, field: CategoryField) {
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

/// Veritabanından okunmuş gelir/gider hareketi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomeExpenseEntry {
    /// Birincil anahtar.
    pub id: i64,
    /// Hareket türü.
    pub entry_type: EntryType,
    /// Bağlı kategorinin kimliği.
    pub category_id: i64,
    /// Kategori adı (liste gösterimi için birleştirilmiş alan).
    pub category_name: String,
    /// Bağlı kasa/banka hesabının kimliği.
    pub cash_account_id: i64,
    /// Kasa/banka hesabı adı (liste gösterimi için birleştirilmiş alan).
    pub cash_account_name: String,
    /// Kayıt tarihi (`YYYY-MM-DD`).
    pub entry_date: String,
    /// Tutar (kuruş, her zaman pozitif).
    pub amount: Money,
    /// Açıklama.
    pub description: Option<String>,
    /// Belge/fiş numarası.
    pub document_ref: Option<String>,
    /// Kayıt durumu.
    pub status: EntryStatus,
    /// İptal zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub cancelled_at: Option<String>,
    /// İptal nedeni.
    pub cancel_reason: Option<String>,
    /// Oluşturulma zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub created_at: String,
    /// Son güncellenme zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub updated_at: String,
}

impl IncomeExpenseEntry {
    /// Kayıt iptal edilmiş mi?
    pub fn is_cancelled(&self) -> bool {
        self.status == EntryStatus::Iptal
    }

    /// Durum etiketi: `Kesin` / `İptal`.
    pub fn status_label(&self) -> &'static str {
        self.status.label()
    }

    /// Tür etiketi: `Gelir` / `Gider`.
    pub fn type_label(&self) -> &'static str {
        self.entry_type.label()
    }

    /// Kasa/banka bakiyesine işaretli etki (gelir `+`, gider `-`).
    pub fn cash_effect(&self) -> Money {
        Money::from_kurus(
            self.amount
                .kurus()
                .saturating_mul(self.entry_type.cash_sign()),
        )
    }

    /// Arama ölçütüne uyuyor mu? (açıklama, kategori adı veya belge no)
    ///
    /// `needle` değeri [`crate::customer::normalize_for_search`] ile normalize
    /// edilmiş olmalıdır.
    pub fn matches_search(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let haystacks = [
            self.description.as_deref(),
            Some(self.category_name.as_str()),
            self.document_ref.as_deref(),
        ];
        haystacks
            .into_iter()
            .flatten()
            .any(|value| normalize_for_search(value).contains(needle))
    }
}

/// Hareket listesi sorgu ölçütleri.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryFilter {
    /// Açıklama, kategori adı veya belge numarasında aranacak metin.
    pub search: String,
    /// Yalnızca bu türü göster (`None` = tümü).
    pub entry_type: Option<EntryType>,
    /// Yalnızca bu kategoriyi göster (`None` = tümü).
    pub category_id: Option<i64>,
    /// Yalnızca bu kasa/banka hesabını göster (`None` = tümü).
    pub cash_account_id: Option<i64>,
    /// Yalnızca bu durumu göster (`None` = tümü).
    pub status: Option<EntryStatus>,
    /// Başlangıç tarihi dahil (`YYYY-MM-DD`, boş = sınır yok).
    pub date_from: String,
    /// Bitiş tarihi dahil (`YYYY-MM-DD`, boş = sınır yok).
    pub date_to: String,
}

impl EntryFilter {
    /// Ölçüt hiçbir şeyi kısıtlamıyor mu?
    pub fn is_empty(&self) -> bool {
        self.search.trim().is_empty()
            && self.entry_type.is_none()
            && self.category_id.is_none()
            && self.cash_account_id.is_none()
            && self.status.is_none()
            && self.date_from.trim().is_empty()
            && self.date_to.trim().is_empty()
    }
}

/// Hareket formundaki ham kullanıcı girdisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryDraft {
    /// Hareket türü.
    pub entry_type: EntryType,
    /// Seçili kategori (`None` = seçilmedi).
    pub category_id: Option<i64>,
    /// Seçili kasa/banka hesabı (`None` = seçilmedi).
    pub cash_account_id: Option<i64>,
    /// Kayıt tarihi (`YYYY-MM-DD`).
    pub entry_date: String,
    /// Tutar (serbest metin; `Money::parse` ile çözülür).
    pub amount: String,
    /// Açıklama.
    pub description: String,
    /// Belge/fiş numarası.
    pub document_ref: String,
}

impl Default for EntryDraft {
    fn default() -> Self {
        Self {
            entry_type: EntryType::default(),
            category_id: None,
            cash_account_id: None,
            entry_date: crate::product::today_iso(),
            amount: String::new(),
            description: String::new(),
            document_ref: String::new(),
        }
    }
}

impl EntryDraft {
    /// Belirli türde boş form üretir.
    pub fn with_type(entry_type: EntryType) -> Self {
        Self {
            entry_type,
            ..Self::default()
        }
    }

    /// Girilen tutarı çözer (`Ok`); boşsa `Ok(ZERO)`, hatalı/negatifse `Err`.
    pub fn input_amount(&self) -> Result<Money, &'static str> {
        if self.amount.trim().is_empty() {
            return Ok(Money::ZERO);
        }
        match Money::parse(&self.amount) {
            Ok(money) if money.is_negative() => Err("Tutar negatif olamaz."),
            Ok(money) => Ok(money),
            Err(_) => Err("Tutar geçerli bir tutar olmalıdır (örnek: 1.234,56)."),
        }
    }

    /// Formdaki veriyi doğrular.
    pub fn validate(&self) -> Result<ValidatedEntry, EntryFieldErrors> {
        let mut errors = EntryFieldErrors::default();

        if self.category_id.is_none() {
            errors.push(EntryField::Category, "Kategori seçilmelidir.");
        }
        if self.cash_account_id.is_none() {
            errors.push(EntryField::CashAccount, "Kasa/banka hesabı seçilmelidir.");
        }

        if chrono::NaiveDate::parse_from_str(self.entry_date.trim(), "%Y-%m-%d").is_err() {
            errors.push(
                EntryField::Date,
                "Tarih geçerli değil (GG.AA.YYYY biçiminde seçin).",
            );
        }

        let amount = match self.input_amount() {
            Ok(money) => {
                if money.is_zero() {
                    errors.push(EntryField::Amount, "Tutar sıfırdan büyük olmalıdır.");
                    None
                } else {
                    Some(money)
                }
            }
            Err(message) => {
                errors.push(EntryField::Amount, message);
                None
            }
        };

        let description = trimmed_optional(&self.description);
        if let Some(text) = &description
            && text.chars().count() > DESCRIPTION_MAX_LEN
        {
            errors.push(
                EntryField::Description,
                format!("Açıklama en fazla {DESCRIPTION_MAX_LEN} karakter olabilir."),
            );
        }

        let document_ref = trimmed_optional(&self.document_ref);
        if let Some(text) = &document_ref
            && text.chars().count() > DOCUMENT_REF_MAX_LEN
        {
            errors.push(
                EntryField::DocumentRef,
                format!("Belge/fiş numarası en fazla {DOCUMENT_REF_MAX_LEN} karakter olabilir."),
            );
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(ValidatedEntry {
            entry_type: self.entry_type,
            category_id: self.category_id.expect("kategori doğrulandı"),
            cash_account_id: self.cash_account_id.expect("kasa hesabı doğrulandı"),
            entry_date: self.entry_date.trim().to_owned(),
            amount: amount.expect("tutar doğrulandı"),
            description,
            document_ref,
        })
    }
}

/// Doğrulamadan geçmiş, veritabanına yazılabilir hareket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedEntry {
    /// Hareket türü.
    pub entry_type: EntryType,
    /// Bağlı kategori.
    pub category_id: i64,
    /// Bağlı kasa/banka hesabı.
    pub cash_account_id: i64,
    /// Kayıt tarihi (`YYYY-MM-DD`).
    pub entry_date: String,
    /// Tutar (kuruş, sıfırdan büyük).
    pub amount: Money,
    /// Açıklama.
    pub description: Option<String>,
    /// Belge/fiş numarası.
    pub document_ref: Option<String>,
}

/// Hareket formunda hata üretilebilen alanlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryField {
    /// Hareket türü.
    Type,
    /// Kategori.
    Category,
    /// Kasa/banka hesabı.
    CashAccount,
    /// Tarih.
    Date,
    /// Tutar.
    Amount,
    /// Açıklama.
    Description,
    /// Belge/fiş numarası.
    DocumentRef,
}

/// Alan bazlı hareket doğrulama hataları.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryFieldErrors {
    entries: Vec<(EntryField, String)>,
}

impl EntryFieldErrors {
    /// Bir alana hata ekler.
    pub fn push(&mut self, field: EntryField, message: impl Into<String>) {
        self.entries.push((field, message.into()));
    }

    /// Alanın hata mesajı (yoksa `None`).
    pub fn get(&self, field: EntryField) -> Option<&str> {
        self.entries
            .iter()
            .find(|(entry, _)| *entry == field)
            .map(|(_, message)| message.as_str())
    }

    /// Alanın hatasını siler.
    pub fn clear(&mut self, field: EntryField) {
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

/// İptal nedenini doğrular (boş olamaz, uzunluk sınırı vardır).
pub fn validate_cancel_reason(reason: &str) -> Result<String, String> {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        return Err("İptal nedeni zorunludur.".to_owned());
    }
    if trimmed.chars().count() > CANCEL_REASON_MAX_LEN {
        return Err(format!(
            "İptal nedeni en fazla {CANCEL_REASON_MAX_LEN} karakter olabilir."
        ));
    }
    Ok(trimmed.to_owned())
}

/// Liste alt toplamları (iptaller hariç).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EntrySummary {
    /// Kesin gelirlerin toplamı.
    pub income_total: Money,
    /// Kesin giderlerin toplamı.
    pub expense_total: Money,
}

impl EntrySummary {
    /// Net (gelir - gider).
    pub fn net(&self) -> Money {
        Money::from_kurus(
            self.income_total
                .kurus()
                .saturating_sub(self.expense_total.kurus()),
        )
    }

    /// İptal olmayan kayıtlardan alt toplam üretir.
    pub fn from_entries<'a, I>(entries: I) -> Self
    where
        I: IntoIterator<Item = &'a IncomeExpenseEntry>,
    {
        let mut summary = Self::default();
        for entry in entries {
            if entry.is_cancelled() {
                continue;
            }
            match entry.entry_type {
                EntryType::Gelir => {
                    summary.income_total = Money::from_kurus(
                        summary
                            .income_total
                            .kurus()
                            .saturating_add(entry.amount.kurus()),
                    );
                }
                EntryType::Gider => {
                    summary.expense_total = Money::from_kurus(
                        summary
                            .expense_total
                            .kurus()
                            .saturating_add(entry.amount.kurus()),
                    );
                }
            }
        }
        summary
    }
}

/// Kategori bazlı özet satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryBreakdown {
    /// Kategori kimliği.
    pub category_id: i64,
    /// Kategori adı.
    pub category_name: String,
    /// Kategori türü.
    pub entry_type: EntryType,
    /// Kesin kayıtların toplamı.
    pub total: Money,
    /// Kendi türünün toplamına göre tam sayı yüzde (`None` = taban sıfır).
    pub percentage: Option<i64>,
}

/// Kasa/banka hesabı bazlı özet satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountBreakdown {
    /// Hesap kimliği.
    pub cash_account_id: i64,
    /// Hesap adı.
    pub cash_account_name: String,
    /// Kesin gelirlerin toplamı.
    pub income_total: Money,
    /// Kesin giderlerin toplamı.
    pub expense_total: Money,
}

impl AccountBreakdown {
    /// Hesabın net etkisi (gelir - gider).
    pub fn net(&self) -> Money {
        Money::from_kurus(
            self.income_total
                .kurus()
                .saturating_sub(self.expense_total.kurus()),
        )
    }
}

/// Seçili dönemin gelir/gider özeti.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeriodSummary {
    /// Dönem başlangıcı (`YYYY-MM-DD`).
    pub date_from: String,
    /// Dönem bitişi (`YYYY-MM-DD`).
    pub date_to: String,
    /// Kesin gelir toplamı.
    pub income_total: Money,
    /// Kesin gider toplamı.
    pub expense_total: Money,
    /// Kategori bazlı dağılım.
    pub categories: Vec<CategoryBreakdown>,
    /// Kasa/banka hesabı bazlı dağılım.
    pub accounts: Vec<AccountBreakdown>,
    /// Dönemdeki **tüm** kayıt sayısı (iptaller dahil).
    pub record_count: usize,
    /// Dönemdeki kesin kayıt sayısı.
    pub confirmed_count: usize,
}

impl PeriodSummary {
    /// Net (gelir - gider).
    pub fn net(&self) -> Money {
        Money::from_kurus(
            self.income_total
                .kurus()
                .saturating_sub(self.expense_total.kurus()),
        )
    }

    /// Dönemde hiç kayıt yok mu? (iptaller dahil)
    pub fn is_empty(&self) -> bool {
        self.record_count == 0
    }

    /// Dönemde kesin kayıt var mı?
    pub fn has_confirmed(&self) -> bool {
        self.confirmed_count > 0
    }

    /// Verilen kayıtlardan dönem özeti üretir.
    ///
    /// Tarih aralığı boş bırakılırsa o yönde sınır uygulanmaz. Özet yalnızca
    /// **kesin** kayıtları toplar; iptaller yalnızca kayıt sayısına dahildir.
    /// Yüzdeler kendi türünün toplamına göre hesaplanır; taban sıfırsa
    /// `None` olur ve arayüz `—` gösterir.
    pub fn from_entries<'a, I>(date_from: &str, date_to: &str, entries: I) -> Self
    where
        I: IntoIterator<Item = &'a IncomeExpenseEntry>,
    {
        let from = date_from.trim();
        let to = date_to.trim();
        let mut summary = Self {
            date_from: from.to_owned(),
            date_to: to.to_owned(),
            ..Self::default()
        };
        let mut categories: std::collections::HashMap<i64, CategoryBreakdown> =
            std::collections::HashMap::new();
        let mut accounts: std::collections::HashMap<i64, AccountBreakdown> =
            std::collections::HashMap::new();

        for entry in entries {
            if !from.is_empty() && entry.entry_date.as_str() < from {
                continue;
            }
            if !to.is_empty() && entry.entry_date.as_str() > to {
                continue;
            }

            summary.record_count += 1;
            if entry.is_cancelled() {
                continue;
            }
            summary.confirmed_count += 1;

            match entry.entry_type {
                EntryType::Gelir => {
                    summary.income_total = Money::from_kurus(
                        summary
                            .income_total
                            .kurus()
                            .saturating_add(entry.amount.kurus()),
                    );
                }
                EntryType::Gider => {
                    summary.expense_total = Money::from_kurus(
                        summary
                            .expense_total
                            .kurus()
                            .saturating_add(entry.amount.kurus()),
                    );
                }
            }

            let category =
                categories
                    .entry(entry.category_id)
                    .or_insert_with(|| CategoryBreakdown {
                        category_id: entry.category_id,
                        category_name: entry.category_name.clone(),
                        entry_type: entry.entry_type,
                        total: Money::ZERO,
                        percentage: None,
                    });
            category.total =
                Money::from_kurus(category.total.kurus().saturating_add(entry.amount.kurus()));

            let account =
                accounts
                    .entry(entry.cash_account_id)
                    .or_insert_with(|| AccountBreakdown {
                        cash_account_id: entry.cash_account_id,
                        cash_account_name: entry.cash_account_name.clone(),
                        income_total: Money::ZERO,
                        expense_total: Money::ZERO,
                    });
            match entry.entry_type {
                EntryType::Gelir => {
                    account.income_total = Money::from_kurus(
                        account
                            .income_total
                            .kurus()
                            .saturating_add(entry.amount.kurus()),
                    );
                }
                EntryType::Gider => {
                    account.expense_total = Money::from_kurus(
                        account
                            .expense_total
                            .kurus()
                            .saturating_add(entry.amount.kurus()),
                    );
                }
            }
        }

        summary.categories = categories.into_values().collect();
        for category in &mut summary.categories {
            let base = match category.entry_type {
                EntryType::Gelir => summary.income_total,
                EntryType::Gider => summary.expense_total,
            };
            category.percentage = category.total.percentage_of(base);
        }
        summary.categories.sort_by(|left, right| {
            left.entry_type
                .cmp(&right.entry_type)
                .then_with(|| right.total.kurus().cmp(&left.total.kurus()))
                .then_with(|| left.category_name.cmp(&right.category_name))
        });

        summary.accounts = accounts.into_values().collect();
        summary
            .accounts
            .sort_by(|left, right| left.cash_account_name.cmp(&right.cash_account_name));

        summary
    }
}

/// İçinde bulunulan ayın başlangıç ve bitiş tarihleri (`YYYY-MM-DD`).
pub fn current_month_range() -> (String, String) {
    use chrono::{Datelike, Local, NaiveDate};

    let today = Local::now().date_naive();
    let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let (next_year, next_month) = if today.month() == 12 {
        (today.year() + 1, 1)
    } else {
        (today.year(), today.month() + 1)
    };
    let next_first = NaiveDate::from_ymd_opt(next_year, next_month, 1).unwrap_or(first);
    let last = next_first.pred_opt().unwrap_or(first);
    (
        first.format("%Y-%m-%d").to_string(),
        last.format("%Y-%m-%d").to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn category_draft(name: &str, category_type: EntryType) -> CategoryDraft {
        CategoryDraft {
            name: name.to_owned(),
            category_type,
            description: String::new(),
            active: true,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn entry(
        id: i64,
        entry_type: EntryType,
        category_id: i64,
        category_name: &str,
        account_id: i64,
        account_name: &str,
        date: &str,
        kurus: i64,
        status: EntryStatus,
    ) -> IncomeExpenseEntry {
        IncomeExpenseEntry {
            id,
            entry_type,
            category_id,
            category_name: category_name.to_owned(),
            cash_account_id: account_id,
            cash_account_name: account_name.to_owned(),
            entry_date: date.to_owned(),
            amount: Money::from_kurus(kurus),
            description: Some("Deneme açıklaması".to_owned()),
            document_ref: Some(format!("FIS-{id}")),
            status,
            cancelled_at: None,
            cancel_reason: None,
            created_at: "2026-10-20 10:00:00".to_owned(),
            updated_at: "2026-10-20 10:00:00".to_owned(),
        }
    }

    #[test]
    fn tur_veritabani_degerleri_ve_kasa_etkisi_dogrudur() {
        assert_eq!(EntryType::Gelir.as_db(), "gelir");
        assert_eq!(EntryType::Gider.as_db(), "gider");
        assert_eq!(EntryType::from_db("gelir"), Some(EntryType::Gelir));
        assert_eq!(EntryType::from_db("gider"), Some(EntryType::Gider));
        assert_eq!(EntryType::from_db("bilinmeyen"), None);
        assert_eq!(EntryType::Gelir.cash_sign(), 1);
        assert_eq!(EntryType::Gider.cash_sign(), -1);
        assert_eq!(EntryType::Gelir.new_label(), "Yeni Gelir");
        assert_eq!(EntryStatus::Iptal.as_db(), "iptal");
    }

    #[test]
    fn kategori_dogrulama_kurallari() {
        let validated = category_draft("  Kira  ", EntryType::Gider)
            .validate()
            .expect("geçerli kategori");
        assert_eq!(validated.name, "Kira");
        assert_eq!(validated.category_type, EntryType::Gider);
        assert!(validated.active);

        let errors = category_draft("   ", EntryType::Gider)
            .validate()
            .expect_err("boş ad reddedilmeli");
        assert_eq!(
            errors.get(CategoryField::Name),
            Some("Kategori adı zorunludur.")
        );

        let long_name = "a".repeat(CATEGORY_NAME_MAX_LEN + 1);
        let errors = category_draft(&long_name, EntryType::Gelir)
            .validate()
            .expect_err("uzun ad reddedilmeli");
        assert!(errors.get(CategoryField::Name).is_some());

        let long_description = "b".repeat(CATEGORY_DESCRIPTION_MAX_LEN + 1);
        let draft = CategoryDraft {
            description: long_description,
            ..category_draft("Kira", EntryType::Gider)
        };
        let errors = draft.validate().expect_err("uzun açıklama reddedilmeli");
        assert!(errors.get(CategoryField::Description).is_some());
    }

    #[test]
    fn kategori_formu_mevcut_kaynaktan_doldurulur() {
        let category = ExpenseCategory {
            id: 7,
            name: "Maaş".to_owned(),
            category_type: EntryType::Gider,
            description: Some("Personel".to_owned()),
            active: false,
            usage_count: 3,
            created_at: "2026-10-01 10:00:00".to_owned(),
            updated_at: "2026-10-01 10:00:00".to_owned(),
        };
        let draft = CategoryDraft::from_category(&category);
        assert_eq!(draft.name, "Maaş");
        assert_eq!(draft.category_type, EntryType::Gider);
        assert_eq!(draft.description, "Personel");
        assert!(!draft.active);
        assert_eq!(category.status_label(), "Pasif");
    }

    #[test]
    fn kategori_aramasi_turkce_karakterleri_normalize_eder() {
        let category = ExpenseCategory {
            id: 1,
            name: "Kırtasiye".to_owned(),
            category_type: EntryType::Gider,
            description: Some("Kalem ve defter".to_owned()),
            active: true,
            usage_count: 0,
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert!(category.matches_search(&normalize_for_search("kirtasiye")));
        assert!(category.matches_search(&normalize_for_search("KALEM")));
        assert!(!category.matches_search(&normalize_for_search("kira")));
    }

    #[test]
    fn hareket_dogrulama_zorunlu_alanlari_ve_tutari_kontrol_eder() {
        let draft = EntryDraft::with_type(EntryType::Gelir);
        let errors = draft.validate().expect_err("boş form geçersiz");
        assert!(errors.get(EntryField::Category).is_some());
        assert!(errors.get(EntryField::CashAccount).is_some());
        assert!(errors.get(EntryField::Amount).is_some());

        let mut draft = EntryDraft::with_type(EntryType::Gider);
        draft.category_id = Some(3);
        draft.cash_account_id = Some(1);
        draft.entry_date = "2026-10-20".to_owned();
        draft.amount = "2.500,00".to_owned();
        draft.description = "  Ekim kirası  ".to_owned();
        draft.document_ref = "  FIS-1 ".to_owned();
        let validated = draft.validate().expect("geçerli hareket");
        assert_eq!(validated.amount, Money::from_kurus(250_000));
        assert_eq!(validated.description.as_deref(), Some("Ekim kirası"));
        assert_eq!(validated.document_ref.as_deref(), Some("FIS-1"));

        draft.amount = "0".to_owned();
        let errors = draft.validate().expect_err("sıfır tutar reddedilmeli");
        assert_eq!(
            errors.get(EntryField::Amount),
            Some("Tutar sıfırdan büyük olmalıdır.")
        );

        draft.amount = "-5".to_owned();
        let errors = draft.validate().expect_err("negatif tutar reddedilmeli");
        assert!(errors.get(EntryField::Amount).is_some());

        draft.amount = "abc".to_owned();
        let errors = draft.validate().expect_err("geçersiz tutar reddedilmeli");
        assert!(errors.get(EntryField::Amount).is_some());

        draft.amount = "10,00".to_owned();
        draft.entry_date = "20.10.2026".to_owned();
        let errors = draft.validate().expect_err("geçersiz tarih reddedilmeli");
        assert!(errors.get(EntryField::Date).is_some());
    }

    #[test]
    fn ozet_iptalleri_haric_toplar_ve_yuzde_tabanini_dogru_secer() {
        let entries = vec![
            entry(
                1,
                EntryType::Gelir,
                10,
                "Kira Geliri",
                1,
                "Merkez Kasa",
                "2026-10-02",
                500_000,
                EntryStatus::Kesin,
            ),
            entry(
                2,
                EntryType::Gelir,
                10,
                "Kira Geliri",
                2,
                "Banka",
                "2026-10-03",
                300_000,
                EntryStatus::Kesin,
            ),
            entry(
                3,
                EntryType::Gider,
                20,
                "Kira",
                1,
                "Merkez Kasa",
                "2026-10-04",
                250_000,
                EntryStatus::Kesin,
            ),
            entry(
                4,
                EntryType::Gider,
                21,
                "Maaş",
                1,
                "Merkez Kasa",
                "2026-10-05",
                750_000,
                EntryStatus::Iptal,
            ),
            entry(
                5,
                EntryType::Gider,
                20,
                "Kira",
                2,
                "Banka",
                "2026-09-30",
                100_000,
                EntryStatus::Kesin,
            ),
        ];

        let summary = PeriodSummary::from_entries("2026-10-01", "2026-10-31", &entries);
        assert_eq!(summary.income_total, Money::from_kurus(800_000));
        assert_eq!(summary.expense_total, Money::from_kurus(250_000));
        assert_eq!(summary.net(), Money::from_kurus(550_000));
        assert_eq!(summary.record_count, 4, "Eylül kaydı dönem dışında");
        assert_eq!(summary.confirmed_count, 3, "iptal kesin sayılmaz");

        let rent_income = summary
            .categories
            .iter()
            .find(|row| row.category_id == 10)
            .expect("kira geliri satırı");
        assert_eq!(rent_income.total, Money::from_kurus(800_000));
        assert_eq!(
            rent_income.percentage,
            Some(100),
            "gelir kategorisi kendi türünün toplamına göre"
        );

        let rent = summary
            .categories
            .iter()
            .find(|row| row.category_id == 20)
            .expect("kira gideri satırı");
        assert_eq!(rent.total, Money::from_kurus(250_000));
        assert_eq!(
            rent.percentage,
            Some(100),
            "gider kategorisi gider toplamına göre"
        );

        let account = summary
            .accounts
            .iter()
            .find(|row| row.cash_account_id == 1)
            .expect("kasa satırı");
        assert_eq!(account.income_total, Money::from_kurus(500_000));
        assert_eq!(account.expense_total, Money::from_kurus(250_000));
        assert_eq!(account.net(), Money::from_kurus(250_000));

        // Yalnızca iptal kayıt varsa kesin toplam sıfır, taban sıfır => yüzde yok.
        let only_cancelled = vec![entry(
            9,
            EntryType::Gider,
            20,
            "Kira",
            1,
            "Merkez Kasa",
            "2026-10-10",
            100,
            EntryStatus::Iptal,
        )];
        let summary = PeriodSummary::from_entries("2026-10-01", "2026-10-31", &only_cancelled);
        assert_eq!(summary.record_count, 1);
        assert_eq!(summary.confirmed_count, 0);
        assert!(!summary.is_empty());
        assert!(!summary.has_confirmed());
        assert!(summary.categories.is_empty());
        assert_eq!(
            summary.income_total.percentage_of(summary.income_total),
            None
        );
    }

    #[test]
    fn filtreler_bos_durumlarini_bildirir() {
        assert!(EntryFilter::default().is_empty());
        assert!(CategoryFilter::default().is_empty());

        let filter = EntryFilter {
            search: "kira".to_owned(),
            ..EntryFilter::default()
        };
        assert!(!filter.is_empty());

        let filter = CategoryFilter {
            active: Some(false),
            ..CategoryFilter::default()
        };
        assert!(!filter.is_empty());
    }

    #[test]
    fn iptal_nedeni_zorunludur_ve_kirpilir() {
        assert_eq!(
            validate_cancel_reason("   ").expect_err("boş neden"),
            "İptal nedeni zorunludur."
        );
        assert_eq!(
            validate_cancel_reason("  Yanlış tutar  ").unwrap(),
            "Yanlış tutar"
        );
        let long = "a".repeat(CANCEL_REASON_MAX_LEN + 1);
        assert!(validate_cancel_reason(&long).is_err());
    }

    #[test]
    fn aylik_aralik_ayin_ilk_ve_son_gunudur() {
        let (from, to) = current_month_range();
        assert!(from.ends_with("-01"));
        assert_eq!(from.len(), 10);
        assert_eq!(to.len(), 10);
        assert!(from <= to);
    }
}
