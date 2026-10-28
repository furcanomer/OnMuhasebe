// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Alış ve satış faturası iş modeli, hesaplama ve doğrulama kuralları.
//!
//! Bu modül **saf iş mantığıdır**: veritabanına ve arayüze bağımlılığı yoktur,
//! bu nedenle birim testleriyle eksiksiz doğrulanabilir.
//!
//! ## Tipler ve akış
//!
//! | Tip                     | Nerede kullanılır                                    |
//! |-------------------------|------------------------------------------------------|
//! | [`InvoiceDraft`]        | Arayüz formundaki ham (doğrulanmamış) kullanıcı girdisi |
//! | [`ValidatedInvoice`]    | Doğrulamadan geçmiş, veritabanına yazılabilir belge  |
//! | [`Invoice`]             | Veritabanından okunmuş, kimliği olan belge           |
//!
//! Geçersiz veri SQL katmanına **hiç ulaşmaz**: veritabanı yalnızca
//! [`ValidatedInvoice`] kabul eder.
//!
//! ## Muhasebe kuralları
//!
//! * Tüm tutarlar **kuruş**, miktarlar **mili-birim** cinsinden tam sayıdır
//!   (bkz. [`crate::money`] ve [`crate::quantity`]).
//! * Satır hesabı sırayla yapılır: brüt -> iskonto -> net -> KDV -> toplam.
//!   Yuvarlamada **yarım kuruş yukarı** gider.
//! * Fatura toplamları satırlardan türetilir; ayrıca saklanan değerle
//!   tutarsızlık oluşmaz.
//! * Belge **silinmez/düzenlenmez**: iptal edilir ve düzeltme yeni bir belgeyle
//!   yapılır.
//!
//! ## Stok yönü
//!
//! Alış faturası stoğu **artırır**, satış faturası **azaltır** (bkz.
//! [`InvoiceType::stock_sign`]).

use std::collections::HashSet;

use chrono::{Datelike, NaiveDate};

use crate::error::AppError;
use crate::money::Money;
use crate::product::{MovementType, Unit};
use crate::quantity::Quantity;

/// Fatura numarasının en fazla uzunluğu (karakter).
pub const NUMBER_MAX_LEN: usize = 32;
/// Fatura notlarının en fazla uzunluğu.
pub const NOTES_MAX_LEN: usize = 1000;
/// Satır açıklamasının en fazla uzunluğu.
pub const LINE_DESCRIPTION_MAX_LEN: usize = 200;
/// İptal nedeninin en fazla uzunluğu.
pub const CANCEL_REASON_MAX_LEN: usize = 500;

/// Faturanın türü.
///
/// Tür, stok yönünü ve numara serisini belirler; belge kesinleştikten sonra
/// **değiştirilemez**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InvoiceType {
    /// Alış faturası: tedarikçiden mal alımı (stoğu artırır).
    #[default]
    Purchase,
    /// Satış faturası: müşteriye mal satışı (stoğu azaltır).
    Sales,
}

impl InvoiceType {
    /// Seçim listesindeki tüm türler.
    pub const ALL: [Self; 2] = [Self::Purchase, Self::Sales];

    /// Kısa tür adı (`"Alış"` / `"Satış"`).
    pub fn label(self) -> &'static str {
        match self {
            Self::Purchase => "Alış",
            Self::Sales => "Satış",
        }
    }

    /// Liste ekranı başlığı.
    pub fn list_title(self) -> &'static str {
        match self {
            Self::Purchase => "Alış Faturaları",
            Self::Sales => "Satış Faturaları",
        }
    }

    /// Veritabanında saklanan değer.
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Purchase => "alis",
            Self::Sales => "satis",
        }
    }

    /// Veritabanındaki değerden türü çözer.
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "alis" => Some(Self::Purchase),
            "satis" => Some(Self::Sales),
            _ => None,
        }
    }

    /// Otomatik numara serisinin öneki (`"AF-"` / `"SF-"`).
    pub fn number_prefix(self) -> &'static str {
        match self {
            Self::Purchase => "AF-",
            Self::Sales => "SF-",
        }
    }

    /// Stoğa etki yönü: alış `+1`, satış `-1`.
    pub fn stock_sign(self) -> i64 {
        match self {
            Self::Purchase => 1,
            Self::Sales => -1,
        }
    }

    /// Bu tür fatura için üretilen stok hareketi türü.
    pub fn movement_type(self) -> MovementType {
        match self {
            Self::Purchase => MovementType::PurchaseInvoice,
            Self::Sales => MovementType::SalesInvoice,
        }
    }

    /// Bu fatura türü ile uyumlu cari türü mü?
    ///
    /// Satışta müşteri, alışta tedarikçi beklenir. Uyumsuz tür **engellenmez**;
    /// arayüz yalnızca uyarı gösterir.
    pub fn is_customer_kind_suitable(self, kind: crate::customer::CustomerType) -> bool {
        use crate::customer::CustomerType;
        match self {
            Self::Sales => matches!(kind, CustomerType::Customer | CustomerType::Both),
            Self::Purchase => matches!(kind, CustomerType::Supplier | CustomerType::Both),
        }
    }
}

/// Faturanın durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InvoiceStatus {
    /// Kesinleşmiş belge (stok ve cari bakiyesine yansımıştır).
    #[default]
    Confirmed,
    /// İptal edilmiş belge (etkisi ters kayıtlarla geri alınmıştır).
    Cancelled,
}

impl InvoiceStatus {
    /// Tüm durumlar (filtre listesi sırası).
    pub const ALL: [Self; 2] = [Self::Confirmed, Self::Cancelled];

    /// Arayüzde gösterilen Türkçe ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Confirmed => "Kesin",
            Self::Cancelled => "İptal",
        }
    }

    /// Veritabanında saklanan değer.
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Confirmed => "kesin",
            Self::Cancelled => "iptal",
        }
    }

    /// Veritabanındaki değerden durumu çözer.
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "kesin" => Some(Self::Confirmed),
            "iptal" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// Bir faturanın tahsilat/ödeme açısından durumu (rozet gösterimi için).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaymentState {
    /// Henüz hiç ödeme bağlanmamış.
    Unpaid,
    /// Bir kısmı ödenmiş, kalanı var.
    Partial,
    /// Tamamı ödenmiş.
    Paid,
}

impl PaymentState {
    /// Arayüzde gösterilen kısa etiket.
    pub fn label(self) -> &'static str {
        match self {
            Self::Unpaid => "Ödenmemiş",
            Self::Partial => "Kısmi",
            Self::Paid => "Ödendi",
        }
    }
}

/// Tek bir fatura satırının hesaplanmış tutarları (kuruş).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineAmounts {
    /// Brüt tutar: `miktar × birim fiyat`.
    pub gross: Money,
    /// İskonto tutarı.
    pub discount: Money,
    /// İskontodan sonraki net tutar (KDV hariç).
    pub net: Money,
    /// KDV tutarı.
    pub vat: Money,
    /// Satır toplamı: net + KDV.
    pub total: Money,
}

/// Tek bir satırın tutarlarını hesaplar.
///
/// Sıra: brüt -> iskonto -> net -> KDV -> toplam. Yuvarlamada yarım kuruş
/// **yukarı** gider; ara sonuçlar `i128` ile hesaplanır ve taşma durumunda
/// hata döner.
pub fn calculate_line(
    quantity: Quantity,
    unit_price: Money,
    discount_rate: i64,
    vat_rate: i64,
) -> std::result::Result<LineAmounts, AppError> {
    let gross = unit_price.checked_mul_quantity(quantity)?;
    let discount = gross.checked_mul_percent(discount_rate)?;
    let net = gross.checked_sub(discount)?;
    let vat = net.checked_mul_percent(vat_rate)?;
    let total = net.checked_add(vat)?;

    Ok(LineAmounts {
        gross,
        discount,
        net,
        vat,
        total,
    })
}

/// Faturanın hesaplanmış toplamları (kuruş).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InvoiceTotals {
    /// Satırların brüt toplamı (iskonto öncesi).
    pub subtotal: Money,
    /// Toplam iskonto.
    pub discount_total: Money,
    /// KDV hariç matrah.
    pub net_total: Money,
    /// Toplam KDV.
    pub vat_total: Money,
    /// Genel toplam (net + KDV).
    pub grand_total: Money,
}

impl InvoiceTotals {
    /// Satırlardan toplamları türetir.
    pub fn from_lines(lines: &[ValidatedInvoiceLine]) -> std::result::Result<Self, AppError> {
        Ok(Self {
            subtotal: Money::checked_sum(lines.iter().map(|line| line.amounts.gross))?,
            discount_total: Money::checked_sum(lines.iter().map(|line| line.amounts.discount))?,
            net_total: Money::checked_sum(lines.iter().map(|line| line.amounts.net))?,
            vat_total: Money::checked_sum(lines.iter().map(|line| line.amounts.vat))?,
            grand_total: Money::checked_sum(lines.iter().map(|line| line.amounts.total))?,
        })
    }
}

/// Veritabanından okunmuş fatura belgesi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invoice {
    /// Birincil anahtar.
    pub id: i64,
    /// Benzersiz fatura numarası.
    pub number: String,
    /// Fatura türü.
    pub invoice_type: InvoiceType,
    /// Bağlı cari hesabın kimliği.
    pub customer_id: i64,
    /// Cari kodu (liste gösterimi için birleştirilmiş alan).
    pub customer_code: String,
    /// Cari adı (liste gösterimi için birleştirilmiş alan).
    pub customer_name: String,
    /// Fatura tarihi (`YYYY-MM-DD`).
    pub invoice_date: String,
    /// Vade tarihi (`YYYY-MM-DD`, isteğe bağlı).
    pub due_date: Option<String>,
    /// Notlar.
    pub notes: Option<String>,
    /// Hesaplanmış toplamlar.
    pub totals: InvoiceTotals,
    /// Bu faturaya bağlanmış kesin tahsilat/ödeme dağıtımlarının toplamı (kuruş).
    ///
    /// İptal edilen tahsilat/ödeme dağıtımları buraya yansımaz; ödeme iptal
    /// edildiğinde değer geri düşürülür.
    pub paid_amount: Money,
    /// Belge durumu.
    pub status: InvoiceStatus,
    /// İptal zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub cancelled_at: Option<String>,
    /// İptal nedeni.
    pub cancel_reason: Option<String>,
    /// Oluşturulma zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub created_at: String,
    /// Son güncellenme zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub updated_at: String,
}

impl Invoice {
    /// Belge iptal edilmiş mi?
    pub fn is_cancelled(&self) -> bool {
        self.status == InvoiceStatus::Cancelled
    }

    /// Arama ölçütüne uyuyor mu? (numara, cari kodu veya cari adı)
    pub fn matches_search(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        [
            self.number.as_str(),
            self.customer_code.as_str(),
            self.customer_name.as_str(),
        ]
        .into_iter()
        .any(|value| crate::customer::normalize_for_search(value).contains(needle))
    }

    /// Faturadan kalan (ödenmemiş) tutar: `grand_total - paid_amount`.
    ///
    /// Ödenen tutar toplamı aşsa bile sonuç negatife düşürülmez (0 döner).
    pub fn remaining(&self) -> Money {
        let raw = self.totals.grand_total.kurus() - self.paid_amount.kurus();
        Money::from_kurus(raw.max(0))
    }

    /// Tahsilat/ödeme rozeti durumu; iptal edilmiş fatura için [`None`].
    pub fn payment_state(&self) -> Option<PaymentState> {
        if self.is_cancelled() {
            return None;
        }
        if self.paid_amount <= Money::ZERO {
            Some(PaymentState::Unpaid)
        } else if self.paid_amount >= self.totals.grand_total {
            Some(PaymentState::Paid)
        } else {
            Some(PaymentState::Partial)
        }
    }
}

/// Veritabanından okunmuş fatura satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceLine {
    /// Birincil anahtar.
    pub id: i64,
    /// Bağlı faturanın kimliği.
    pub invoice_id: i64,
    /// Bağlı ürünün kimliği.
    pub product_id: i64,
    /// Satır sırası (1'den başlar).
    pub line_no: i64,
    /// Ürün kodu (belge kesilirken kopyalanmıştır).
    pub product_code: String,
    /// Ürün adı (belge kesilirken kopyalanmıştır).
    pub product_name: String,
    /// Ölçü birimi (belge kesilirken kopyalanmıştır).
    pub unit: Unit,
    /// Satır açıklaması.
    pub description: Option<String>,
    /// Miktar (mili-birim, her zaman pozitif).
    pub quantity: Quantity,
    /// Birim fiyat (kuruş).
    pub unit_price: Money,
    /// İskonto oranı (0-100).
    pub discount_rate: i64,
    /// KDV oranı (0-100).
    pub vat_rate: i64,
    /// Hesaplanmış satır tutarları.
    pub amounts: LineAmounts,
}

impl InvoiceLine {
    /// Miktar, birim etiketiyle (`"2,5 kg"`).
    pub fn formatted_quantity(&self) -> String {
        self.quantity.format_with_label(self.unit.short_label())
    }

    /// İskonto oranı etiketi (`"%10"`).
    pub fn discount_label(&self) -> String {
        format!("%{}", self.discount_rate)
    }

    /// KDV oranı etiketi (`"%20"`).
    pub fn vat_label(&self) -> String {
        format!("%{}", self.vat_rate)
    }
}

/// Doğrulamadan geçmiş, veritabanına yazılabilir fatura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedInvoice {
    /// Fatura numarası (kırpılmış).
    pub number: String,
    /// Fatura türü.
    pub invoice_type: InvoiceType,
    /// Bağlı cari hesabın kimliği.
    pub customer_id: i64,
    /// Fatura tarihi (`YYYY-MM-DD`).
    pub invoice_date: String,
    /// Vade tarihi (`YYYY-MM-DD`, isteğe bağlı).
    pub due_date: Option<String>,
    /// Notlar.
    pub notes: Option<String>,
    /// Doğrulanmış satırlar (en az bir tane).
    pub lines: Vec<ValidatedInvoiceLine>,
    /// Satırlardan türetilmiş toplamlar.
    pub totals: InvoiceTotals,
}

impl ValidatedInvoice {
    /// Satır sayısı.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Tüm satırların toplam miktarı.
    pub fn total_quantity(&self) -> Quantity {
        self.lines.iter().fold(Quantity::ZERO, |total, line| {
            total.checked_add(line.quantity).unwrap_or(total)
        })
    }
}

/// Doğrulamadan geçmiş fatura satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedInvoiceLine {
    /// Bağlı ürünün kimliği.
    pub product_id: i64,
    /// Ürün kodu (belgeye kopyalanır).
    pub product_code: String,
    /// Ürün adı (belgeye kopyalanır).
    pub product_name: String,
    /// Ölçü birimi (belgeye kopyalanır).
    pub unit: Unit,
    /// Satır açıklaması (isteğe bağlı).
    pub description: Option<String>,
    /// Miktar (mili-birim, pozitif).
    pub quantity: Quantity,
    /// Birim fiyat (kuruş).
    pub unit_price: Money,
    /// İskonto oranı (0-100).
    pub discount_rate: i64,
    /// KDV oranı (0-100).
    pub vat_rate: i64,
    /// Hesaplanmış satır tutarları.
    pub amounts: LineAmounts,
}

/// Fatura listesi sorgu ölçütleri.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvoiceFilter {
    /// Numara, cari kodu veya cari adında aranacak metin.
    pub search: String,
    /// Yalnızca bu cariyi göster (`None` = tümü).
    pub customer_id: Option<i64>,
    /// Yalnızca bu durumu göster (`None` = tümü).
    pub status: Option<InvoiceStatus>,
    /// Başlangıç tarihi (`YYYY-MM-DD`, boş = sınır yok).
    pub date_from: String,
    /// Bitiş tarihi (`YYYY-MM-DD`, boş = sınır yok).
    pub date_to: String,
}

impl InvoiceFilter {
    /// Ölçüt hiçbir şeyi kısıtlıyor mu?
    pub fn is_empty(&self) -> bool {
        self.search.trim().is_empty()
            && self.customer_id.is_none()
            && self.status.is_none()
            && self.date_from.trim().is_empty()
            && self.date_to.trim().is_empty()
    }
}

/// Arayüz formundaki tek bir fatura satırının ham girdisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceLineDraft {
    /// Seçilen ürünün kimliği.
    pub product_id: Option<i64>,
    /// Ürün kodu (ürün seçilince kopyalanır, elle değiştirilemez).
    pub product_code: String,
    /// Ürün adı (ürün seçilince kopyalanır, elle değiştirilemez).
    pub product_name: String,
    /// Ölçü birimi (ürün kartından gelir, elle değiştirilemez).
    pub unit: Unit,
    /// Serbest açıklama.
    pub description: String,
    /// Miktar (serbest metin; `Quantity::parse` ile çözümlenir).
    pub quantity: String,
    /// Birim fiyat (serbest metin; `Money::parse` ile çözümlenir).
    pub unit_price: String,
    /// İskonto oranı (0-100; boş = 0).
    pub discount_rate: String,
    /// KDV oranı (0-100).
    pub vat_rate: String,
}

impl Default for InvoiceLineDraft {
    fn default() -> Self {
        Self {
            product_id: None,
            product_code: String::new(),
            product_name: String::new(),
            unit: Unit::default(),
            description: String::new(),
            quantity: String::new(),
            unit_price: String::new(),
            discount_rate: String::new(),
            vat_rate: "20".to_owned(),
        }
    }
}

impl InvoiceLineDraft {
    /// Satırın canlı önizleme tutarları; girdi geçersizse `None` döner.
    pub fn preview_amounts(&self) -> Option<LineAmounts> {
        let quantity = Quantity::parse(&self.quantity).ok()?;
        if !quantity.is_positive() {
            return None;
        }
        let unit_price = Money::parse(&self.unit_price).ok()?;
        if unit_price.is_negative() {
            return None;
        }
        let discount_rate = if self.discount_rate.trim().is_empty() {
            0
        } else {
            parse_rate(&self.discount_rate).ok()?
        };
        let vat_rate = parse_rate(&self.vat_rate).ok()?;
        calculate_line(quantity, unit_price, discount_rate, vat_rate).ok()
    }
}

/// Arayüz formundaki ham fatura girdisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceDraft {
    /// Fatura numarası.
    pub number: String,
    /// Fatura türü.
    pub invoice_type: InvoiceType,
    /// Seçilen cari hesabın kimliği.
    pub customer_id: Option<i64>,
    /// Fatura tarihi (`YYYY-MM-DD`).
    pub invoice_date: String,
    /// Vade tarihi (`YYYY-MM-DD`, boş olabilir).
    pub due_date: String,
    /// Notlar.
    pub notes: String,
    /// Fatura satırları (en az bir taneyle başlar).
    pub lines: Vec<InvoiceLineDraft>,
}

impl Default for InvoiceDraft {
    fn default() -> Self {
        Self::new(InvoiceType::default())
    }
}

impl InvoiceDraft {
    /// Belirtilen türde, bugünün tarihli ve tek boş satırlı yeni taslak.
    pub fn new(invoice_type: InvoiceType) -> Self {
        Self {
            number: String::new(),
            invoice_type,
            customer_id: None,
            invoice_date: crate::product::today_iso(),
            due_date: String::new(),
            notes: String::new(),
            lines: vec![InvoiceLineDraft::default()],
        }
    }

    /// Formdaki geçerli satırlardan **canlı** toplam önizlemesi üretir.
    ///
    /// Geçersiz veya eksik satırlar yok sayılır; bu yalnızca kullanıcıya
    /// geri bildirim içindir, kaydedilen değerler doğrulama sonucunda üretilir.
    pub fn preview_totals(&self) -> InvoiceTotals {
        let mut totals = InvoiceTotals::default();
        for line in &self.lines {
            let Some(amounts) = line.preview_amounts() else {
                continue;
            };
            totals.subtotal = totals
                .subtotal
                .checked_add(amounts.gross)
                .unwrap_or(totals.subtotal);
            totals.discount_total = totals
                .discount_total
                .checked_add(amounts.discount)
                .unwrap_or(totals.discount_total);
            totals.net_total = totals
                .net_total
                .checked_add(amounts.net)
                .unwrap_or(totals.net_total);
            totals.vat_total = totals
                .vat_total
                .checked_add(amounts.vat)
                .unwrap_or(totals.vat_total);
            totals.grand_total = totals
                .grand_total
                .checked_add(amounts.total)
                .unwrap_or(totals.grand_total);
        }
        totals
    }

    /// Formu doğrular ve veritabanına yazılabilir belgeye dönüştürür.
    ///
    /// Birden fazla hata varsa **hepsi** birden döner; satır hataları satır
    /// numarasıyla birlikte taşınır ki arayüz hatayı doğru satırın altında
    /// gösterebilsin.
    pub fn validate(&self) -> std::result::Result<ValidatedInvoice, FieldErrors> {
        let mut errors = FieldErrors::default();

        let number = self.number.trim().to_owned();
        if number.is_empty() {
            errors.push(Field::Number, "Fatura numarası zorunludur.");
        } else if number.chars().count() > NUMBER_MAX_LEN {
            errors.push(
                Field::Number,
                format!("Fatura numarası en fazla {NUMBER_MAX_LEN} karakter olabilir."),
            );
        } else if number.chars().any(char::is_whitespace) {
            errors.push(Field::Number, "Fatura numarası boşluk içeremez.");
        }

        let date = self.invoice_date.trim();
        if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            errors.push(Field::Date, "Tarih geçerli olmalıdır (biçim: 2026-10-18).");
        }

        let due_date = self.due_date.trim();
        if !due_date.is_empty() && NaiveDate::parse_from_str(due_date, "%Y-%m-%d").is_err() {
            errors.push(
                Field::DueDate,
                "Vade tarihi geçerli olmalıdır (biçim: 2026-10-18).",
            );
        }

        if self.customer_id.is_none() {
            errors.push(Field::Customer, "Cari hesap seçilmelidir.");
        }

        let notes = self.notes.trim();
        if notes.chars().count() > NOTES_MAX_LEN {
            errors.push(
                Field::Notes,
                format!("Notlar en fazla {NOTES_MAX_LEN} karakter olabilir."),
            );
        }

        if self.lines.is_empty() {
            errors.push(Field::Lines, "En az bir fatura satırı girilmelidir.");
        }

        let mut lines = Vec::with_capacity(self.lines.len());
        for (index, line) in self.lines.iter().enumerate() {
            match line.validate(index) {
                Ok(validated) => lines.push(validated),
                Err(line_errors) => errors.merge(line_errors),
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let totals = match InvoiceTotals::from_lines(&lines) {
            Ok(totals) => totals,
            Err(_) => {
                errors.push(
                    Field::Lines,
                    "Tutar toplamı hesaplanamadı (tutarlar çok büyük).",
                );
                return Err(errors);
            }
        };

        // Cari, yukarıda doğrulandı; buraya yalnızca seçili olduğunda gelinir.
        let Some(customer_id) = self.customer_id else {
            return Err(errors);
        };

        Ok(ValidatedInvoice {
            number,
            invoice_type: self.invoice_type,
            customer_id,
            invoice_date: date.to_owned(),
            due_date: (!due_date.is_empty()).then(|| due_date.to_owned()),
            notes: (!notes.is_empty()).then(|| notes.to_owned()),
            lines,
            totals,
        })
    }
}

impl InvoiceLineDraft {
    /// Tek bir satırı doğrular.
    fn validate(&self, index: usize) -> std::result::Result<ValidatedInvoiceLine, FieldErrors> {
        let mut errors = FieldErrors::default();

        if self.product_id.is_none() {
            errors.push_line(index, Field::LineProduct, "Ürün seçilmelidir.");
        }

        let quantity = match Quantity::parse(&self.quantity) {
            Ok(quantity) if quantity.is_positive() => Some(quantity),
            Ok(_) => {
                errors.push_line(
                    index,
                    Field::LineQuantity,
                    "Miktar sıfırdan büyük olmalıdır.",
                );
                None
            }
            Err(_) => {
                errors.push_line(
                    index,
                    Field::LineQuantity,
                    "Miktar geçerli bir sayı olmalıdır.",
                );
                None
            }
        };

        let unit_price = match Money::parse(&self.unit_price) {
            Ok(price) if !price.is_negative() => Some(price),
            Ok(_) => {
                errors.push_line(index, Field::LineUnitPrice, "Birim fiyat negatif olamaz.");
                None
            }
            Err(_) => {
                errors.push_line(
                    index,
                    Field::LineUnitPrice,
                    "Birim fiyat geçerli bir tutar olmalıdır.",
                );
                None
            }
        };

        let discount_rate = if self.discount_rate.trim().is_empty() {
            Some(0)
        } else {
            match parse_rate(&self.discount_rate) {
                Ok(rate) => Some(rate),
                Err(message) => {
                    errors.push_line(index, Field::LineDiscount, message);
                    None
                }
            }
        };

        let vat_rate = match parse_rate(&self.vat_rate) {
            Ok(rate) => Some(rate),
            Err(message) => {
                errors.push_line(index, Field::LineVat, message);
                None
            }
        };

        let description = self.description.trim();
        if description.chars().count() > LINE_DESCRIPTION_MAX_LEN {
            errors.push_line(
                index,
                Field::LineDescription,
                format!("Açıklama en fazla {LINE_DESCRIPTION_MAX_LEN} karakter olabilir."),
            );
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let (quantity, unit_price, discount_rate, vat_rate) = (
            quantity.expect("miktar doğrulandı"),
            unit_price.expect("birim fiyat doğrulandı"),
            discount_rate.expect("iskonto doğrulandı"),
            vat_rate.expect("KDV doğrulandı"),
        );

        let amounts = match calculate_line(quantity, unit_price, discount_rate, vat_rate) {
            Ok(amounts) => amounts,
            Err(error) => {
                errors.push_line(index, Field::LineUnitPrice, error.to_string());
                return Err(errors);
            }
        };

        Ok(ValidatedInvoiceLine {
            product_id: self.product_id.expect("ürün doğrulandı"),
            product_code: self.product_code.clone(),
            product_name: self.product_name.clone(),
            unit: self.unit,
            description: (!description.is_empty()).then(|| description.to_owned()),
            quantity,
            unit_price,
            discount_rate,
            vat_rate,
            amounts,
        })
    }
}

/// Fatura formundaki hata taşıyan alanlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// Fatura numarası.
    Number,
    /// Fatura tarihi.
    Date,
    /// Vade tarihi.
    DueDate,
    /// Cari hesap seçimi.
    Customer,
    /// Notlar.
    Notes,
    /// Genel satır listesi (örneğin hiç satır yok).
    Lines,
    /// Satır: ürün seçimi.
    LineProduct,
    /// Satır: miktar.
    LineQuantity,
    /// Satır: birim fiyat.
    LineUnitPrice,
    /// Satır: iskonto oranı.
    LineDiscount,
    /// Satır: KDV oranı.
    LineVat,
    /// Satır: açıklama.
    LineDescription,
    /// İptal nedeni.
    CancelReason,
}

/// Alan bazlı hata toplayıcı (satır hataları satır numarasıyla birlikte).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldErrors {
    header: Vec<(Field, String)>,
    lines: Vec<(usize, Field, String)>,
}

impl FieldErrors {
    /// Form geneli (satıra bağlı olmayan) bir hata ekler.
    pub fn push(&mut self, field: Field, message: impl Into<String>) {
        self.header.push((field, message.into()));
    }

    /// Belirli bir satıra ait hata ekler.
    pub fn push_line(&mut self, line: usize, field: Field, message: impl Into<String>) {
        self.lines.push((line, field, message.into()));
    }

    /// Başka bir hata kümesini bu kümeye katar.
    pub fn merge(&mut self, other: Self) {
        self.header.extend(other.header);
        self.lines.extend(other.lines);
    }

    /// Form geneli bir alanın ilk hata mesajı.
    pub fn get(&self, field: Field) -> Option<&str> {
        self.header
            .iter()
            .find(|(candidate, _)| *candidate == field)
            .map(|(_, message)| message.as_str())
    }

    /// Belirli bir satırdaki alanın ilk hata mesajı.
    pub fn get_line(&self, line: usize, field: Field) -> Option<&str> {
        self.lines
            .iter()
            .find(|(candidate, candidate_field, _)| *candidate == line && *candidate_field == field)
            .map(|(_, _, message)| message.as_str())
    }

    /// Satırda herhangi bir hata var mı?
    pub fn has_line(&self, line: usize) -> bool {
        self.lines
            .iter()
            .any(|(candidate, _, _)| *candidate == line)
    }

    /// Form geneli bir alanın hatasını siler.
    pub fn clear(&mut self, field: Field) {
        self.header.retain(|(candidate, _)| *candidate != field);
    }

    /// Belirli bir satırdaki bir alanın hatasını siler.
    pub fn clear_line(&mut self, line: usize, field: Field) {
        self.lines.retain(|(candidate, candidate_field, _)| {
            !(*candidate == line && *candidate_field == field)
        });
    }

    /// Bir satıra ait tüm hataları siler (satır değiştiğinde/silindiğinde).
    pub fn clear_line_all(&mut self, line: usize) {
        self.lines.retain(|(candidate, _, _)| *candidate != line);
    }

    /// Tüm satır hatalarını siler (satır sırası değiştiğinde).
    pub fn clear_lines(&mut self) {
        self.lines.clear();
    }

    /// Hiç hata yok mu?
    pub fn is_empty(&self) -> bool {
        self.header.is_empty() && self.lines.is_empty()
    }

    /// Toplam hata sayısı.
    pub fn len(&self) -> usize {
        self.header.len() + self.lines.len()
    }
}

/// Fatura tarihinden yılı çıkarır; tarih geçersizse bugünün yılını kullanır.
pub fn year_of(date: &str) -> i32 {
    NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d")
        .map(|parsed| parsed.year())
        .unwrap_or_else(|_| chrono::Local::now().date_naive().year())
}

/// Var olan numaralara bakarak seri/yıl için kullanılmamış bir numara önerir.
///
/// Örnek: `SF-2026-0004`. Öneri yalnızca kolaylık sağlar; benzersizliğin asıl
/// güvencesi veritabanındaki `UNIQUE` kısıtıdır.
pub fn suggest_next_number(
    existing_numbers: &[String],
    invoice_type: InvoiceType,
    year: i32,
) -> String {
    let prefix = format!("{}{year}-", invoice_type.number_prefix());
    let normalized_prefix = crate::customer::normalize_for_search(&prefix);
    let used: HashSet<String> = existing_numbers
        .iter()
        .map(|number| crate::customer::normalize_for_search(number))
        .collect();

    let highest = existing_numbers
        .iter()
        .filter_map(|number| {
            let normalized = crate::customer::normalize_for_search(number);
            normalized
                .strip_prefix(&normalized_prefix)
                .and_then(|suffix| suffix.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);

    let first = highest.saturating_add(1);
    for offset in 0..100_000_u32 {
        let candidate = format!("{prefix}{:04}", first.saturating_add(offset));
        if !used.contains(&crate::customer::normalize_for_search(&candidate)) {
            return candidate;
        }
    }

    format!("{prefix}{first:04}")
}

/// İptal nedenini doğrular ve kırpılmış metni döndürür.
pub fn validate_cancel_reason(reason: &str) -> Result<String, String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("İptal nedeni zorunludur.".to_owned());
    }
    if reason.chars().count() > CANCEL_REASON_MAX_LEN {
        return Err(format!(
            "İptal nedeni en fazla {CANCEL_REASON_MAX_LEN} karakter olabilir."
        ));
    }
    Ok(reason.to_owned())
}

/// Bir oranı (0-100 tam sayı) çözer.
fn parse_rate(input: &str) -> std::result::Result<i64, String> {
    let trimmed = input.trim();
    let message = "Oran 0-100 arasında bir tam sayı olmalıdır.".to_owned();
    let value: i64 = trimmed.parse().map_err(|_| message.clone())?;
    if !(0..=100).contains(&value) {
        return Err(message);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::customer::CustomerType;

    fn line_draft() -> InvoiceLineDraft {
        InvoiceLineDraft {
            product_id: Some(1),
            product_code: "URN-0001".to_owned(),
            product_name: "Vida".to_owned(),
            unit: Unit::Adet,
            description: String::new(),
            quantity: "2".to_owned(),
            unit_price: "10".to_owned(),
            discount_rate: "0".to_owned(),
            vat_rate: "20".to_owned(),
        }
    }

    fn draft() -> InvoiceDraft {
        InvoiceDraft {
            number: "SF-2026-0001".to_owned(),
            invoice_type: InvoiceType::Sales,
            customer_id: Some(7),
            invoice_date: "2026-10-18".to_owned(),
            due_date: String::new(),
            notes: String::new(),
            lines: vec![line_draft()],
        }
    }

    #[test]
    fn tur_ve_durum_veritabani_karsiligi_tutarlidir() {
        for invoice_type in InvoiceType::ALL {
            assert!(!invoice_type.label().is_empty());
            assert!(!invoice_type.list_title().is_empty());
            assert_eq!(
                InvoiceType::from_db(invoice_type.as_db()),
                Some(invoice_type)
            );
        }
        assert_eq!(InvoiceType::from_db("bilinmeyen"), None);

        for status in InvoiceStatus::ALL {
            assert!(!status.label().is_empty());
            assert_eq!(InvoiceStatus::from_db(status.as_db()), Some(status));
        }
        assert_eq!(InvoiceStatus::from_db("bilinmeyen"), None);
    }

    #[test]
    fn stok_yonu_ve_hareket_turu_tutarlidir() {
        assert_eq!(InvoiceType::Purchase.stock_sign(), 1);
        assert_eq!(InvoiceType::Sales.stock_sign(), -1);
        assert_eq!(
            InvoiceType::Purchase.movement_type(),
            MovementType::PurchaseInvoice
        );
        assert_eq!(
            InvoiceType::Sales.movement_type(),
            MovementType::SalesInvoice
        );
    }

    #[test]
    fn cari_turu_uygunlugu_dogru_bildirilir() {
        assert!(InvoiceType::Sales.is_customer_kind_suitable(CustomerType::Customer));
        assert!(InvoiceType::Sales.is_customer_kind_suitable(CustomerType::Both));
        assert!(!InvoiceType::Sales.is_customer_kind_suitable(CustomerType::Supplier));

        assert!(InvoiceType::Purchase.is_customer_kind_suitable(CustomerType::Supplier));
        assert!(InvoiceType::Purchase.is_customer_kind_suitable(CustomerType::Both));
        assert!(!InvoiceType::Purchase.is_customer_kind_suitable(CustomerType::Customer));
    }

    #[test]
    fn satir_hesabi_iskonto_ve_kdv_ile_dogru_calisir() {
        // 2 adet × 10,00 ₺ = 20,00 ₺ brüt; %10 iskonto -> 18,00 ₺; %20 KDV -> 3,60 ₺; toplam 21,60 ₺.
        let amounts =
            calculate_line(Quantity::from_milli(2000), Money::from_kurus(1000), 10, 20).unwrap();
        assert_eq!(amounts.gross.kurus(), 2000);
        assert_eq!(amounts.discount.kurus(), 200);
        assert_eq!(amounts.net.kurus(), 1800);
        assert_eq!(amounts.vat.kurus(), 360);
        assert_eq!(amounts.total.kurus(), 2160);
    }

    #[test]
    fn satir_hesabi_ondalikli_miktarda_yarim_kurusu_yukari_yuvarlar() {
        // 1,5 × 1,00 ₺ = 1,50 ₺; %0 iskonto; %20 KDV = 0,30 ₺; toplam 1,80 ₺.
        let amounts =
            calculate_line(Quantity::from_milli(1500), Money::from_kurus(100), 0, 20).unwrap();
        assert_eq!(amounts.gross.kurus(), 150);
        assert_eq!(amounts.vat.kurus(), 30);
        assert_eq!(amounts.total.kurus(), 180);

        // 0,005 × 1,00 ₺ = 0,5 kuruş -> 1 kuruş (yarım yukarı).
        let amounts =
            calculate_line(Quantity::from_milli(5), Money::from_kurus(100), 0, 0).unwrap();
        assert_eq!(amounts.gross.kurus(), 1);
    }

    #[test]
    fn satir_hesabi_tasmayi_yakalar() {
        assert!(
            calculate_line(
                Quantity::from_milli(2_000_000),
                Money::from_kurus(i64::MAX),
                0,
                20,
            )
            .is_err(),
            "taşan hesap hata vermeli"
        );
    }

    #[test]
    fn gecerli_form_dogrulamadan_gecer() {
        let validated = draft().validate().expect("doğrulama geçmeli");
        assert_eq!(validated.number, "SF-2026-0001");
        assert_eq!(validated.customer_id, 7);
        assert_eq!(validated.invoice_date, "2026-10-18");
        assert!(validated.due_date.is_none());
        assert!(validated.notes.is_none());
        assert_eq!(validated.line_count(), 1);
        assert_eq!(validated.totals.grand_total.kurus(), 2400);
        assert_eq!(validated.totals.net_total.kurus(), 2000);
        assert_eq!(validated.totals.vat_total.kurus(), 400);
    }

    #[test]
    fn zorunlu_alanlar_ve_satir_hatalari_tek_seferde_doner() {
        let mut form = draft();
        form.number = "  ".to_owned();
        form.customer_id = None;
        form.invoice_date = "18.10.2026".to_owned();
        form.lines[0].product_id = None;
        form.lines[0].quantity = "0".to_owned();
        form.lines[0].unit_price = "abc".to_owned();
        form.lines[0].vat_rate = "150".to_owned();

        let errors = form.validate().expect_err("doğrulama geçmemeli");
        assert!(errors.get(Field::Number).is_some());
        assert!(errors.get(Field::Customer).is_some());
        assert!(errors.get(Field::Date).is_some());
        assert!(errors.get_line(0, Field::LineProduct).is_some());
        assert!(errors.get_line(0, Field::LineQuantity).is_some());
        assert!(errors.get_line(0, Field::LineUnitPrice).is_some());
        assert!(errors.get_line(0, Field::LineVat).is_some());
        assert!(errors.has_line(0));
        assert!(!errors.has_line(1));
    }

    #[test]
    fn hata_mesaji_temizlenince_duzelir() {
        let mut form = draft();
        form.number = String::new();
        let mut errors = form.validate().expect_err("doğrulama geçmemeli");
        assert!(errors.get(Field::Number).is_some());
        errors.clear(Field::Number);
        assert!(errors.get(Field::Number).is_none());

        form.lines[0].quantity = "0".to_owned();
        let mut errors = form.validate().expect_err("doğrulama geçmemeli");
        assert!(errors.get_line(0, Field::LineQuantity).is_some());
        errors.clear_line(0, Field::LineQuantity);
        assert!(errors.get_line(0, Field::LineQuantity).is_none());
        errors.clear_line_all(0);
        assert!(!errors.has_line(0));
    }

    #[test]
    fn satir_yoksa_hata_doner() {
        let mut form = draft();
        form.lines.clear();
        let errors = form
            .validate()
            .expect_err("satırsız fatura kabul edilmemeli");
        assert!(errors.get(Field::Lines).is_some());
    }

    #[test]
    fn onizleme_toplamlari_gecerli_satirlari_toplar() {
        let mut form = draft();
        form.lines.push(InvoiceLineDraft {
            quantity: "1".to_owned(),
            unit_price: "5".to_owned(),
            discount_rate: String::new(),
            vat_rate: "0".to_owned(),
            ..line_draft()
        });
        // Geçersiz satır yok sayılır.
        form.lines.push(InvoiceLineDraft {
            quantity: "abc".to_owned(),
            ..line_draft()
        });

        let totals = form.preview_totals();
        // İlk satır: brüt 20,00 ₺ (KDV %20) -> toplam 24,00 ₺.
        // İkinci satır: brüt 5,00 ₺ (KDV %0) -> toplam 5,00 ₺.
        assert_eq!(totals.subtotal.kurus(), 2500);
        assert_eq!(totals.grand_total.kurus(), 2900);
    }

    #[test]
    fn numara_onerisi_seri_ve_yili_dikkate_alir() {
        let existing = vec![
            "SF-2026-0001".to_owned(),
            "sf-2026-0003".to_owned(),
            "AF-2026-0009".to_owned(),
        ];
        assert_eq!(
            suggest_next_number(&existing, InvoiceType::Sales, 2026),
            "SF-2026-0004"
        );
        // Alış serisi ayrı sayılır.
        assert_eq!(
            suggest_next_number(&existing, InvoiceType::Purchase, 2026),
            "AF-2026-0010"
        );
        // Yeni yıl sıfırdan başlar.
        assert_eq!(
            suggest_next_number(&existing, InvoiceType::Sales, 2027),
            "SF-2027-0001"
        );
    }

    #[test]
    fn numara_onerisi_bos_kullanilan_numarayi_atlar() {
        let existing = vec!["SF-2026-0001".to_owned(), "SF-2026-0002".to_owned()];
        assert_eq!(
            suggest_next_number(&existing, InvoiceType::Sales, 2026),
            "SF-2026-0003"
        );
    }

    #[test]
    fn yil_tarihten_cikarilir() {
        assert_eq!(year_of("2026-10-18"), 2026);
        assert_eq!(year_of(" 2030-01-01 "), 2030);
        // Geçersiz tarihte bugünün yılı kullanılır.
        assert!(year_of("gecersiz") >= 2020);
    }

    #[test]
    fn iptal_nedeni_zorunludur() {
        assert!(validate_cancel_reason("   ").is_err());
        assert_eq!(
            validate_cancel_reason("  Müşteri vazgeçti. ").unwrap(),
            "Müşteri vazgeçti."
        );
    }

    #[test]
    fn arama_numara_ve_cari_bilgisinde_calisir() {
        let invoice = Invoice {
            id: 1,
            number: "SF-2026-0001".to_owned(),
            invoice_type: InvoiceType::Sales,
            customer_id: 7,
            customer_code: "CAR-0007".to_owned(),
            customer_name: "Öztürk İnşaat".to_owned(),
            invoice_date: "2026-10-18".to_owned(),
            due_date: None,
            notes: None,
            totals: InvoiceTotals::default(),
            paid_amount: Money::ZERO,
            status: InvoiceStatus::Confirmed,
            cancelled_at: None,
            cancel_reason: None,
            created_at: "2026-10-18 10:00:00".to_owned(),
            updated_at: "2026-10-18 10:00:00".to_owned(),
        };
        assert!(invoice.matches_search(&crate::customer::normalize_for_search("ozturk")));
        assert!(invoice.matches_search(&crate::customer::normalize_for_search("sf-2026")));
        assert!(!invoice.matches_search(&crate::customer::normalize_for_search("yilmaz")));
        assert!(!invoice.is_cancelled());
    }

    #[test]
    fn filtre_bos_mu_bilgisi_dogrudur() {
        let mut filter = InvoiceFilter::default();
        assert!(filter.is_empty());
        filter.status = Some(InvoiceStatus::Cancelled);
        assert!(!filter.is_empty());
        filter.status = None;
        filter.date_from = "2026-01-01".to_owned();
        assert!(!filter.is_empty());
    }
}
