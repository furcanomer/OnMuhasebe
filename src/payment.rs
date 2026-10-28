// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Tahsilat/ödeme iş modeli, dağıtım kuralları ve doğrulama.
//!
//! Bu modül **saf iş mantığıdır**: veritabanına ve arayüze bağımlılığı yoktur,
//! bu nedenle birim testleriyle eksiksiz doğrulanabilir.
//!
//! ## Tipler ve akış
//!
//! | Tip                 | Nerede kullanılır                                     |
//! |---------------------|-------------------------------------------------------|
//! | [`PaymentDraft`]    | Arayüz formundaki ham (doğrulanmamış) kullanıcı girdisi |
//! | [`ValidatedPayment`] | Doğrulamadan geçmiş, veritabanına yazılabilir kayıt  |
//! | [`Payment`]         | Veritabanından okunmuş, kimliği olan kayıt            |
//!
//! ## Kurallar
//!
//! * Tutar **sıfırdan büyük** olmalıdır.
//! * Bir tahsilat/ödeme **tek bir** kasa/banka hesabına bağlıdır.
//! * Dağıtımların toplamı tutarı **aşamaz**; eşit olabilir. Aradaki fark
//!   avans sayılır ve hiç dağıtım yapılmadan da kayıt kesinleşebilir.
//! * Tür uyuşmazlığı (tahsilat ↔ tedarikçi) **engellenmez**; arayüz yalnızca
//!   uyarı gösterir.
//! * Kayıt **silinmez/düzenlenmez**: düzeltme, iptal edip yeni bir kayıt
//!   oluşturmakla yapılır.

use crate::customer::CustomerType;
use crate::invoice::InvoiceType;
use crate::money::Money;

/// Açıklama için azami uzunluk.
pub const DESCRIPTION_MAX_LEN: usize = 500;

/// İptal nedeni için azami uzunluk.
pub const CANCEL_REASON_MAX_LEN: usize = 500;

/// Tahsilat/ödeme türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PaymentType {
    /// Tahsilat: müşteriden para girişi (kasa/banka bakiyesi artar).
    #[default]
    Tahsilat,
    /// Ödeme: tedarikçiye para çıkışı (kasa/banka bakiyesi azalır).
    Odeme,
}

impl PaymentType {
    /// Seçim listesindeki tüm türler.
    pub const ALL: [Self; 2] = [Self::Tahsilat, Self::Odeme];

    /// Arayüzde gösterilen Türkçe ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Tahsilat => "Tahsilat",
            Self::Odeme => "Ödeme",
        }
    }

    /// Kaydet düğmesi metni (`"Tahsilatı Kaydet"` / `"Ödemeyi Kaydet"`).
    pub fn save_label(self) -> &'static str {
        match self {
            Self::Tahsilat => "Tahsilatı Kaydet",
            Self::Odeme => "Ödemeyi Kaydet",
        }
    }

    /// Veritabanında saklanan değer.
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Tahsilat => "tahsilat",
            Self::Odeme => "odeme",
        }
    }

    /// Veritabanındaki değerden türü çözer.
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "tahsilat" => Some(Self::Tahsilat),
            "odeme" => Some(Self::Odeme),
            _ => None,
        }
    }

    /// Bu türün dağıtılabileceği fatura türü.
    ///
    /// Tahsilat yalnızca **satış**, ödeme yalnızca **alış** faturasına
    /// dağıtılabilir.
    pub fn invoice_type(self) -> InvoiceType {
        match self {
            Self::Tahsilat => InvoiceType::Sales,
            Self::Odeme => InvoiceType::Purchase,
        }
    }

    /// Kasa/banka bakiyesine etki yönü: tahsilat `+1`, ödeme `-1`.
    pub fn cash_sign(self) -> i64 {
        match self {
            Self::Tahsilat => 1,
            Self::Odeme => -1,
        }
    }

    /// Bu tür için beklenen cari türüyle uyumlu mu?
    ///
    /// Tahsilatta müşteri, ödemede tedarikçi beklenir. Uyumsuz tür
    /// **engellenmez**; arayüz yalnızca uyarı gösterir.
    pub fn is_customer_kind_suitable(self, kind: CustomerType) -> bool {
        match self {
            Self::Tahsilat => matches!(kind, CustomerType::Customer | CustomerType::Both),
            Self::Odeme => matches!(kind, CustomerType::Supplier | CustomerType::Both),
        }
    }

    /// Cari türü uyumsuzsa gösterilecek uyarı metni.
    pub fn customer_kind_warning(self, kind: CustomerType) -> Option<String> {
        if self.is_customer_kind_suitable(kind) {
            return None;
        }
        let expected = match self {
            Self::Tahsilat => "müşteri",
            Self::Odeme => "tedarikçi",
        };
        Some(format!(
            "Seçilen cari '{}' türünde. {} için genellikle '{}' türü beklenir; yine de kaydedebilirsiniz.",
            kind.label(),
            self.label(),
            expected
        ))
    }
}

/// Tahsilat/ödeme kaydının durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PaymentStatus {
    /// Kesinleşmiş kayıt (kasa ve faturalara yansımıştır).
    #[default]
    Kesin,
    /// İptal edilmiş kayıt (etkisi geri alınmıştır).
    Iptal,
}

impl PaymentStatus {
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

/// Veritabanından okunmuş tahsilat/ödeme kaydı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payment {
    /// Birincil anahtar.
    pub id: i64,
    /// Tahsilat/ödeme türü.
    pub payment_type: PaymentType,
    /// Bağlı cari hesabın kimliği.
    pub customer_id: i64,
    /// Cari kodu (liste gösterimi için birleştirilmiş alan).
    pub customer_code: String,
    /// Cari adı (liste gösterimi için birleştirilmiş alan).
    pub customer_name: String,
    /// Bağlı kasa/banka hesabının kimliği.
    pub cash_account_id: i64,
    /// Kasa/banka hesabı adı (liste gösterimi için birleştirilmiş alan).
    pub cash_account_name: String,
    /// Kayıt tarihi (`YYYY-MM-DD`).
    pub payment_date: String,
    /// Tutar (kuruş, her zaman pozitif).
    pub amount: Money,
    /// Açıklama.
    pub description: Option<String>,
    /// Kayıt durumu.
    pub status: PaymentStatus,
    /// İptal zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub cancelled_at: Option<String>,
    /// İptal nedeni.
    pub cancel_reason: Option<String>,
    /// Oluşturulma zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub created_at: String,
    /// Son güncellenme zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub updated_at: String,
    /// Bu kaydın faturalara dağıttığı toplam tutar (kuruş).
    pub allocated_total: Money,
    /// Bu kayda bağlı dağıtım sayısı.
    pub allocation_count: usize,
}

impl Payment {
    /// Kayıt iptal edilmiş mi?
    pub fn is_cancelled(&self) -> bool {
        self.status == PaymentStatus::Iptal
    }

    /// Dağıtılmayan (avans) tutar: `amount - allocated_total`.
    pub fn remainder(&self) -> Money {
        let raw = self.amount.kurus() - self.allocated_total.kurus();
        Money::from_kurus(raw.max(0))
    }

    /// Bir avans kaydı mı? (hiç dağıtım yapılmamış)
    pub fn is_advance(&self) -> bool {
        self.allocation_count == 0
    }

    /// Durum etiketi: `Kesin` / `İptal`.
    pub fn status_label(&self) -> &'static str {
        self.status.label()
    }

    /// Arama ölçütüne uyuyor mu? (açıklama, cari adı veya kodu)
    ///
    /// `needle` değeri [`crate::customer::normalize_for_search`] ile normalize
    /// edilmiş olmalıdır.
    pub fn matches_search(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let haystacks = [
            self.description.as_deref(),
            Some(self.customer_name.as_str()),
            Some(self.customer_code.as_str()),
        ];
        haystacks
            .into_iter()
            .flatten()
            .any(|value| crate::customer::normalize_for_search(value).contains(needle))
    }
}

/// Veritabanından okunmuş ödeme dağıtımı (ödeme ayrıntısı için).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentAllocation {
    /// Birincil anahtar.
    pub id: i64,
    /// Bağlı tahsilat/ödeme kaydının kimliği.
    pub payment_id: i64,
    /// Bağlı faturanın kimliği.
    pub invoice_id: i64,
    /// Fatura numarası.
    pub invoice_number: String,
    /// Fatura tarihi (`YYYY-MM-DD`).
    pub invoice_date: String,
    /// Faturanın genel toplamı (kuruş).
    pub invoice_grand_total: Money,
    /// Bu ödemeden faturaya ayrılan tutar (kuruş).
    pub amount: Money,
}

/// Bir faturaya bağlı tahsilat/ödeme bağlantısı (fatura ayrıntısı için).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoicePaymentLink {
    /// Bağlı tahsilat/ödeme kaydının kimliği.
    pub payment_id: i64,
    /// Kayıt türü.
    pub payment_type: PaymentType,
    /// Kayıt tarihi (`YYYY-MM-DD`).
    pub payment_date: String,
    /// Kasa/banka hesabı adı.
    pub cash_account_name: String,
    /// Bu faturaya ayrılan tutar (kuruş).
    pub amount: Money,
    /// Kaydın durumu.
    pub status: PaymentStatus,
}

/// Tahsilat/ödeme listesi sorgu ölçütleri.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaymentFilter {
    /// Açıklama veya cari adında aranacak metin.
    pub search: String,
    /// Yalnızca bu cariyi göster (`None` = tümü).
    pub customer_id: Option<i64>,
    /// Yalnızca bu kasa/banka hesabını göster (`None` = tümü).
    pub cash_account_id: Option<i64>,
    /// Yalnızca bu türü göster (`None` = tümü).
    pub payment_type: Option<PaymentType>,
    /// Yalnızca bu durumu göster (`None` = tümü).
    pub status: Option<PaymentStatus>,
    /// Başlangıç tarihi dâhil (`YYYY-MM-DD`, boş = sınır yok).
    pub date_from: String,
    /// Bitiş tarihi dâhil (`YYYY-MM-DD`, boş = sınır yok).
    pub date_to: String,
}

impl PaymentFilter {
    /// Ölçüt hiçbir şeyi kısıtlamıyor mu?
    pub fn is_empty(&self) -> bool {
        self.search.trim().is_empty()
            && self.customer_id.is_none()
            && self.cash_account_id.is_none()
            && self.payment_type.is_none()
            && self.status.is_none()
            && self.date_from.trim().is_empty()
            && self.date_to.trim().is_empty()
    }
}

/// Liste alt toplamları (iptaller hariç).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaymentSummary {
    /// Kesin tahsilatların toplamı.
    pub collection_total: Money,
    /// Kesin ödemelerin toplamı.
    pub payment_total: Money,
}

impl PaymentSummary {
    /// Net (tahsilat - ödeme).
    pub fn net(&self) -> Money {
        Money::from_kurus(
            self.collection_total
                .kurus()
                .saturating_sub(self.payment_total.kurus()),
        )
    }

    /// İptal olmayan kayıtlardan alt toplam üretir.
    pub fn from_payments<'a, I>(payments: I) -> Self
    where
        I: IntoIterator<Item = &'a Payment>,
    {
        let mut summary = Self::default();
        for payment in payments {
            if payment.is_cancelled() {
                continue;
            }
            match payment.payment_type {
                PaymentType::Tahsilat => {
                    summary.collection_total = Money::from_kurus(
                        summary
                            .collection_total
                            .kurus()
                            .saturating_add(payment.amount.kurus()),
                    );
                }
                PaymentType::Odeme => {
                    summary.payment_total = Money::from_kurus(
                        summary
                            .payment_total
                            .kurus()
                            .saturating_add(payment.amount.kurus()),
                    );
                }
            }
        }
        summary
    }
}

/// Ödeme formundaki tek bir fatura satırının (dağıtım) ham girdisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationDraft {
    /// Bağlı faturanın kimliği.
    pub invoice_id: i64,
    /// Fatura numarası.
    pub invoice_number: String,
    /// Fatura tarihi (`YYYY-MM-DD`).
    pub invoice_date: String,
    /// Faturanın genel toplamı (kuruş).
    pub grand_total: Money,
    /// Faturaya şu ana kadar ödenmiş tutar (kuruş).
    pub paid_amount: Money,
    /// Kullanıcının bu faturaya dağıtmak istediği tutar (serbest metin).
    pub amount: String,
}

impl AllocationDraft {
    /// Faturadan kalan (dağıtılabilir) tutar.
    pub fn remaining(&self) -> Money {
        let raw = self.grand_total.kurus() - self.paid_amount.kurus();
        Money::from_kurus(raw.max(0))
    }

    /// Girilen dağıtım tutarını çözer.
    ///
    /// Boş girdi `Money::ZERO` olur; negatif veya çözülemeyen girdi hata döner.
    pub fn input_amount(&self) -> Result<Money, &'static str> {
        if self.amount.trim().is_empty() {
            return Ok(Money::ZERO);
        }
        match Money::parse(&self.amount) {
            Ok(money) if money.is_negative() => Err("Dağıtılacak tutar negatif olamaz."),
            Ok(money) => Ok(money),
            Err(_) => Err("Dağıtılacak tutar geçerli bir tutar olmalıdır (örnek: 1.234,56)."),
        }
    }
}

/// Tahsilat/ödeme formundaki ham kullanıcı girdisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentDraft {
    /// Kayıt türü.
    pub payment_type: PaymentType,
    /// Seçili cari hesap (`None` = seçilmedi).
    pub customer_id: Option<i64>,
    /// Seçili kasa/banka hesabı (`None` = seçilmedi).
    pub cash_account_id: Option<i64>,
    /// Kayıt tarihi (`YYYY-MM-DD`).
    pub payment_date: String,
    /// Tutar (serbest metin; `Money::parse` ile çözülür).
    pub amount: String,
    /// Açıklama.
    pub description: String,
    /// Fatura dağıtımları.
    pub allocations: Vec<AllocationDraft>,
}

impl Default for PaymentDraft {
    /// Yeni kayıt formunun başlangıç değerleri.
    fn default() -> Self {
        Self {
            payment_type: PaymentType::default(),
            customer_id: None,
            cash_account_id: None,
            payment_date: crate::product::today_iso(),
            amount: String::new(),
            description: String::new(),
            allocations: Vec::new(),
        }
    }
}

impl PaymentDraft {
    /// Fatura listesini (dağıtım panelini) yenisiyle değiştirir.
    pub fn replace_allocations(&mut self, allocations: Vec<AllocationDraft>) {
        self.allocations = allocations;
    }

    /// Tüm dağıtımları kalan tutara eşitler ("Tümünü dağıt").
    pub fn allocate_all(&mut self) {
        for allocation in &mut self.allocations {
            allocation.amount = allocation.remaining().format_turkish();
        }
    }

    /// Tüm dağıtım miktarlarını temizler.
    pub fn clear_amounts(&mut self) {
        for allocation in &mut self.allocations {
            allocation.amount.clear();
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

    /// Girilen dağıtımların toplamı; herhangi biri geçersizse `None`.
    pub fn allocation_input_total(&self) -> Option<Money> {
        let mut total = Money::ZERO;
        for allocation in &self.allocations {
            let value = allocation.input_amount().ok()?;
            total = total.checked_add(value).ok()?;
        }
        Some(total)
    }

    /// Kalan (avans) tutar; girdiler geçersizse `None`.
    pub fn input_remainder(&self) -> Option<Money> {
        let amount = self.input_amount().ok()?;
        let allocated = self.allocation_input_total()?;
        Some(Money::from_kurus(
            amount.kurus().saturating_sub(allocated.kurus()),
        ))
    }

    /// Formdaki veriyi doğrular ve veritabanına yazılabilir kayda dönüştürür.
    pub fn validate(&self) -> Result<ValidatedPayment, FieldErrors> {
        let mut errors = FieldErrors::default();

        if self.customer_id.is_none() {
            errors.push(Field::Customer, "Cari hesap seçilmelidir.");
        }
        if self.cash_account_id.is_none() {
            errors.push(Field::CashAccount, "Kasa/banka hesabı seçilmelidir.");
        }

        if chrono::NaiveDate::parse_from_str(self.payment_date.trim(), "%Y-%m-%d").is_err() {
            errors.push(
                Field::Date,
                "Tarih geçerli değil (GG.AA.YYYY biçiminde seçin).",
            );
        }

        let amount = match self.input_amount() {
            Ok(money) => {
                if money.is_zero() {
                    errors.push(Field::Amount, "Tutar sıfırdan büyük olmalıdır.");
                    None
                } else {
                    Some(money)
                }
            }
            Err(message) => {
                errors.push(Field::Amount, message);
                None
            }
        };

        let description = crate::customer::trimmed_optional(&self.description);
        if let Some(text) = &description
            && text.chars().count() > DESCRIPTION_MAX_LEN
        {
            errors.push(
                Field::Description,
                format!("Açıklama en fazla {DESCRIPTION_MAX_LEN} karakter olabilir."),
            );
        }

        let mut allocations = Vec::new();
        let mut allocated_total = Money::ZERO;
        let mut overflow = false;

        for (index, allocation) in self.allocations.iter().enumerate() {
            match allocation.input_amount() {
                Err(message) => errors.push_allocation(index, message),
                Ok(money) => {
                    if money.is_zero() {
                        continue;
                    }
                    if money.is_negative() {
                        errors.push_allocation(index, "Dağıtılacak tutar negatif olamaz.");
                        continue;
                    }
                    if money > allocation.remaining() {
                        errors.push_allocation(
                            index,
                            format!(
                                "Kalan tutarı ({}) aşıyor.",
                                allocation.remaining().format_with_symbol()
                            ),
                        );
                        continue;
                    }
                    match allocated_total.checked_add(money) {
                        Ok(total) => allocated_total = total,
                        Err(_) => overflow = true,
                    }
                    allocations.push(ValidatedAllocation {
                        invoice_id: allocation.invoice_id,
                        amount: money,
                    });
                }
            }
        }

        if overflow {
            errors.push(Field::Allocations, "Dağıtılan toplam çok büyük.");
        }

        if let Some(amount) = amount {
            match self.allocation_input_total() {
                Some(input_total) if input_total > amount => {
                    errors.push(
                        Field::Allocations,
                        format!(
                            "Dağıtılan toplam ({}) ödeme tutarını ({}) aşıyor.",
                            input_total.format_with_symbol(),
                            amount.format_with_symbol()
                        ),
                    );
                }
                None => errors.push(
                    Field::Allocations,
                    "Dağıtım tutarlarından biri geçerli değil.",
                ),
                _ => {}
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(ValidatedPayment {
            payment_type: self.payment_type,
            customer_id: self.customer_id.expect("cari doğrulandı"),
            cash_account_id: self.cash_account_id.expect("kasa hesabı doğrulandı"),
            payment_date: self.payment_date.trim().to_owned(),
            amount: amount.expect("tutar doğrulandı"),
            description,
            allocations,
        })
    }
}

/// Doğrulamadan geçmiş, veritabanına yazılabilir tahsilat/ödeme kaydı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPayment {
    /// Kayıt türü.
    pub payment_type: PaymentType,
    /// Bağlı cari hesabın kimliği.
    pub customer_id: i64,
    /// Bağlı kasa/banka hesabının kimliği.
    pub cash_account_id: i64,
    /// Kayıt tarihi (`YYYY-MM-DD`).
    pub payment_date: String,
    /// Tutar (kuruş, sıfırdan büyük).
    pub amount: Money,
    /// Açıklama.
    pub description: Option<String>,
    /// Doğrulanmış dağıtımlar (boş olabilir = avans).
    pub allocations: Vec<ValidatedAllocation>,
}

impl ValidatedPayment {
    /// Dağıtımların toplam tutarı.
    pub fn allocated_total(&self) -> Money {
        Money::checked_sum(self.allocations.iter().map(|allocation| allocation.amount))
            .unwrap_or(Money::ZERO)
    }

    /// Dağıtılmayan (avans) tutar.
    pub fn remainder(&self) -> Money {
        Money::from_kurus(
            self.amount
                .kurus()
                .saturating_sub(self.allocated_total().kurus()),
        )
    }
}

/// Doğrulamadan geçmiş tek bir dağıtım.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedAllocation {
    /// Bağlı faturanın kimliği.
    pub invoice_id: i64,
    /// Faturaya ayrılan tutar (kuruş, sıfırdan büyük).
    pub amount: Money,
}

/// Formda hata üretilebilen alanlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// Cari hesap.
    Customer,
    /// Kasa/banka hesabı.
    CashAccount,
    /// Tarih.
    Date,
    /// Tutar.
    Amount,
    /// Açıklama.
    Description,
    /// Dağıtım toplamı (genel).
    Allocations,
    /// İptal nedeni.
    CancelReason,
}

/// Alan bazlı doğrulama hataları (genel alanlar ve dağıtım satırları).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldErrors {
    entries: Vec<(Field, String)>,
    allocations: Vec<(usize, String)>,
}

impl FieldErrors {
    /// Bir alana hata ekler.
    pub fn push(&mut self, field: Field, message: impl Into<String>) {
        self.entries.push((field, message.into()));
    }

    /// Belirli bir dağıtım satırına (sırasına göre) hata ekler.
    pub fn push_allocation(&mut self, index: usize, message: impl Into<String>) {
        self.allocations.push((index, message.into()));
    }

    /// Alanın hata mesajı (yoksa `None`).
    pub fn get(&self, field: Field) -> Option<&str> {
        self.entries
            .iter()
            .find(|(entry, _)| *entry == field)
            .map(|(_, message)| message.as_str())
    }

    /// Bir dağıtım satırının hata mesajı (yoksa `None`).
    pub fn allocation(&self, index: usize) -> Option<&str> {
        self.allocations
            .iter()
            .find(|(entry, _)| *entry == index)
            .map(|(_, message)| message.as_str())
    }

    /// Alanın hatasını siler.
    pub fn clear(&mut self, field: Field) {
        self.entries.retain(|(entry, _)| *entry != field);
    }

    /// Bir dağıtım satırının hatasını siler.
    pub fn clear_allocation(&mut self, index: usize) {
        self.allocations.retain(|(entry, _)| *entry != index);
    }

    /// Hiç hata yok mu?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.allocations.is_empty()
    }

    /// Hata sayısı.
    pub fn len(&self) -> usize {
        self.entries.len() + self.allocations.len()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn allocation(invoice_id: i64, total: i64, paid: i64, amount: &str) -> AllocationDraft {
        AllocationDraft {
            invoice_id,
            invoice_number: format!("SF-2026-{invoice_id:04}"),
            invoice_date: "2026-10-18".to_owned(),
            grand_total: Money::from_kurus(total),
            paid_amount: Money::from_kurus(paid),
            amount: amount.to_owned(),
        }
    }

    fn draft() -> PaymentDraft {
        PaymentDraft {
            payment_type: PaymentType::Tahsilat,
            customer_id: Some(1),
            cash_account_id: Some(1),
            payment_date: "2026-10-18".to_owned(),
            amount: "100,00".to_owned(),
            description: String::new(),
            allocations: Vec::new(),
        }
    }

    #[test]
    fn tur_etiketleri_ve_veritabani_karsiligi_tutarlidir() {
        for payment_type in PaymentType::ALL {
            assert!(!payment_type.label().is_empty());
            assert_eq!(
                PaymentType::from_db(payment_type.as_db()),
                Some(payment_type)
            );
        }
        assert_eq!(PaymentType::from_db("bilinmeyen"), None);
        assert_eq!(PaymentType::Tahsilat.cash_sign(), 1);
        assert_eq!(PaymentType::Odeme.cash_sign(), -1);
        assert_eq!(PaymentType::Tahsilat.invoice_type(), InvoiceType::Sales);
        assert_eq!(PaymentType::Odeme.invoice_type(), InvoiceType::Purchase);
    }

    #[test]
    fn durum_etiketleri_ve_veritabani_karsiligi_tutarlidir() {
        for status in PaymentStatus::ALL {
            assert!(!status.label().is_empty());
            assert_eq!(PaymentStatus::from_db(status.as_db()), Some(status));
        }
        assert_eq!(PaymentStatus::from_db("bilinmeyen"), None);
    }

    #[test]
    fn cari_turu_uyumu_uyari_uretir_ama_engellemez() {
        assert!(PaymentType::Tahsilat.is_customer_kind_suitable(CustomerType::Customer));
        assert!(PaymentType::Tahsilat.is_customer_kind_suitable(CustomerType::Both));
        assert!(!PaymentType::Tahsilat.is_customer_kind_suitable(CustomerType::Supplier));
        assert!(PaymentType::Odeme.is_customer_kind_suitable(CustomerType::Supplier));
        assert!(!PaymentType::Odeme.is_customer_kind_suitable(CustomerType::Customer));

        assert!(
            PaymentType::Tahsilat
                .customer_kind_warning(CustomerType::Supplier)
                .is_some()
        );
        assert!(
            PaymentType::Tahsilat
                .customer_kind_warning(CustomerType::Customer)
                .is_none()
        );
    }

    #[test]
    fn sifir_tutar_reddedilir() {
        let mut form = draft();
        form.amount = "0".to_owned();
        let errors = form.validate().expect_err("sıfır tutar reddedilmeli");
        assert!(errors.get(Field::Amount).is_some());
    }

    #[test]
    fn negatif_ve_gecersiz_tutar_reddedilir() {
        let mut form = draft();
        form.amount = "-5".to_owned();
        assert!(form.validate().unwrap_err().get(Field::Amount).is_some());

        form.amount = "abc".to_owned();
        assert!(form.validate().unwrap_err().get(Field::Amount).is_some());
    }

    #[test]
    fn cari_ve_kasa_zorunludur() {
        let mut form = draft();
        form.customer_id = None;
        form.cash_account_id = None;
        let errors = form.validate().expect_err("zorunlu alanlar");
        assert!(errors.get(Field::Customer).is_some());
        assert!(errors.get(Field::CashAccount).is_some());
    }

    #[test]
    fn tam_dagitim_tutar_kadar_gecerlidir() {
        let mut form = draft();
        form.amount = "250,00".to_owned();
        form.replace_allocations(vec![allocation(1, 250_000, 0, "250,00")]);
        // 2.500,00 faturaya 250,00 dağıtılır; kalan 0 -> geçerli.
        let validated = form.validate().expect("geçerli olmalı");
        assert_eq!(validated.amount, Money::from_kurus(25_000));
        assert_eq!(validated.allocations.len(), 1);
        assert_eq!(validated.allocated_total(), Money::from_kurus(25_000));
        assert!(validated.remainder().is_zero());
    }

    #[test]
    fn kismi_dagitim_avans_birakir() {
        let mut form = draft();
        form.amount = "100,00".to_owned();
        form.replace_allocations(vec![allocation(1, 250_000, 0, "40,00")]);
        let validated = form.validate().expect("geçerli olmalı");
        assert_eq!(validated.allocated_total(), Money::from_kurus(4_000));
        assert_eq!(validated.remainder(), Money::from_kurus(6_000));
    }

    #[test]
    fn asiri_dagitim_reddedilir() {
        let mut form = draft();
        form.amount = "100,00".to_owned();
        form.replace_allocations(vec![allocation(1, 250_000, 0, "150,00")]);
        let errors = form.validate().expect_err("ödeme tutarını aşan dağıtım");
        assert!(errors.get(Field::Allocations).is_some());
    }

    #[test]
    fn fatura_kalanini_asan_dagitim_reddedilir() {
        let mut form = draft();
        form.amount = "1.000,00".to_owned();
        // Faturada kalan 50,00 (1.000,00 - 950,00); 60,00 dağıtılamaz.
        form.replace_allocations(vec![allocation(1, 100_000, 95_000, "60,00")]);
        let errors = form.validate().expect_err("fatura kalanı aşılamaz");
        assert!(errors.allocation(0).is_some());
    }

    #[test]
    fn dagitimsiz_kayit_avans_olarak_gecerlidir() {
        let mut form = draft();
        form.amount = "500,00".to_owned();
        form.replace_allocations(vec![]);
        let validated = form.validate().expect("avans geçerli olmalı");
        assert!(validated.allocations.is_empty());
        assert_eq!(validated.remainder(), Money::from_kurus(50_000));
    }

    #[test]
    fn bos_dagitim_sifir_sayilir() {
        let mut form = draft();
        form.amount = "100,00".to_owned();
        form.replace_allocations(vec![allocation(1, 250_000, 0, "  ")]);
        let validated = form.validate().expect("boş dağıtım sıfır sayılmalı");
        assert!(validated.allocations.is_empty());
    }

    #[test]
    fn tumunu_dagit_kalanlari_doldurur() {
        let mut form = draft();
        form.replace_allocations(vec![
            allocation(1, 100_000, 40_000, ""),
            allocation(2, 50_000, 0, ""),
        ]);
        form.allocate_all();
        assert_eq!(form.allocations[0].amount, "600,00");
        assert_eq!(form.allocations[1].amount, "500,00");
        assert_eq!(form.allocation_input_total().unwrap().kurus(), 110_000);
    }

    #[test]
    fn iptal_nedeni_zorunludur() {
        assert!(validate_cancel_reason("   ").is_err());
        assert_eq!(
            validate_cancel_reason("  Yanlış kayıt ").unwrap(),
            "Yanlış kayıt"
        );
    }

    #[test]
    fn avans_ve_kalan_hesabi_dogrudur() {
        let payment = Payment {
            id: 1,
            payment_type: PaymentType::Tahsilat,
            customer_id: 1,
            customer_code: "CAR-0001".to_owned(),
            customer_name: "ABC".to_owned(),
            cash_account_id: 1,
            cash_account_name: "Merkez Kasa".to_owned(),
            payment_date: "2026-10-18".to_owned(),
            amount: Money::from_kurus(100_000),
            description: None,
            status: PaymentStatus::Kesin,
            cancelled_at: None,
            cancel_reason: None,
            created_at: "2026-10-18 10:00:00".to_owned(),
            updated_at: "2026-10-18 10:00:00".to_owned(),
            allocated_total: Money::from_kurus(60_000),
            allocation_count: 2,
        };
        assert_eq!(payment.remainder().kurus(), 40_000);
        assert!(!payment.is_advance());
        assert_eq!(payment.status_label(), "Kesin");

        let mut advance = payment.clone();
        advance.allocation_count = 0;
        advance.allocated_total = Money::ZERO;
        assert!(advance.is_advance());
        assert_eq!(advance.remainder().kurus(), 100_000);
    }

    #[test]
    fn liste_ozeti_iptalleri_haric_tutar() {
        let mut tahsilat = Payment {
            id: 1,
            payment_type: PaymentType::Tahsilat,
            customer_id: 1,
            customer_code: "CAR-0001".to_owned(),
            customer_name: "ABC".to_owned(),
            cash_account_id: 1,
            cash_account_name: "Merkez Kasa".to_owned(),
            payment_date: "2026-10-18".to_owned(),
            amount: Money::from_kurus(100_000),
            description: None,
            status: PaymentStatus::Kesin,
            cancelled_at: None,
            cancel_reason: None,
            created_at: String::new(),
            updated_at: String::new(),
            allocated_total: Money::ZERO,
            allocation_count: 0,
        };
        let mut odeme = tahsilat.clone();
        odeme.id = 2;
        odeme.payment_type = PaymentType::Odeme;
        odeme.amount = Money::from_kurus(30_000);
        let mut iptal = tahsilat.clone();
        iptal.id = 3;
        iptal.status = PaymentStatus::Iptal;

        let summary = PaymentSummary::from_payments([&tahsilat, &odeme, &iptal]);
        assert_eq!(summary.collection_total.kurus(), 100_000);
        assert_eq!(summary.payment_total.kurus(), 30_000);
        assert_eq!(summary.net().kurus(), 70_000);

        // İptal edilen kayıt özet dışıdır.
        tahsilat.status = PaymentStatus::Iptal;
        let summary = PaymentSummary::from_payments([&tahsilat, &odeme]);
        assert_eq!(summary.collection_total.kurus(), 0);
    }
}
