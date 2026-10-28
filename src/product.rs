// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ürün ve stok iş modeli ile doğrulama kuralları.
//!
//! Bu modül **saf iş mantığıdır**: veritabanına ve arayüze bağımlılığı yoktur,
//! bu nedenle birim testleriyle eksiksiz doğrulanabilir.
//!
//! Cari hesap modülündeki (bkz. [`crate::customer`]) desen burada da geçerlidir:
//!
//! | Tip                 | Nerede kullanılır                                      |
//! |---------------------|--------------------------------------------------------|
//! | [`ProductDraft`]    | Arayüz formundaki ham (doğrulanmamış) kullanıcı girdisi |
//! | [`ValidatedProduct`]| Doğrulamadan geçmiş, veritabanına yazılabilir kayıt     |
//! | [`Product`]         | Veritabanından okunmuş, kimliği olan kayıt             |
//!
//! ## Stok hakkında
//!
//! Ürünün `current_stock` değeri **yalnızca stok hareketleriyle** değişir ve
//! veritabanında hareketle aynı işlem (transaction) içinde güncellenir. Stok
//! hareketleri doğrudan düzenlenemez veya silinemez; düzeltme, ters yönlü bir
//! `duzeltme` hareketiyle yapılır.

use std::collections::HashSet;

use chrono::NaiveDate;

use crate::money::Money;
use crate::quantity::Quantity;

/// Otomatik üretilen ürün kodu öneki (örnek: `URN-0001`).
pub const CODE_PREFIX: &str = "URN-";

/// Ürün kodunun en fazla uzunluğu (karakter).
pub const CODE_MAX_LEN: usize = 32;
/// Ürün adının en fazla uzunluğu.
pub const NAME_MAX_LEN: usize = 200;
/// Barkodun en az uzunluğu.
pub const BARCODE_MIN_LEN: usize = 4;
/// Barkodun en fazla uzunluğu.
pub const BARCODE_MAX_LEN: usize = 32;
/// Kategori adının en fazla uzunluğu.
pub const CATEGORY_MAX_LEN: usize = 100;
/// Notların en fazla uzunluğu.
pub const NOTES_MAX_LEN: usize = 1000;
/// Stok hareketi açıklamasının en fazla uzunluğu.
pub const MOVEMENT_DESCRIPTION_MAX_LEN: usize = 500;

/// Formda hızlı seçim için sunulan standart KDV oranları.
pub const VAT_RATES: [i64; 6] = [0, 1, 8, 10, 18, 20];

/// Ürünün ölçü birimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Unit {
    /// Adet.
    #[default]
    Adet,
    /// Kilogram.
    Kilogram,
    /// Gram.
    Gram,
    /// Litre.
    Litre,
    /// Mililitre.
    Millilitre,
    /// Metre.
    Metre,
    /// Metrekare.
    Metrekare,
    /// Paket.
    Paket,
    /// Kutu.
    Kutu,
    /// Koli.
    Koli,
    /// Ton.
    Ton,
}

impl Unit {
    /// Seçim listesinde gösterilecek sıradaki tüm birimler.
    pub const ALL: [Self; 11] = [
        Self::Adet,
        Self::Kilogram,
        Self::Gram,
        Self::Litre,
        Self::Millilitre,
        Self::Metre,
        Self::Metrekare,
        Self::Paket,
        Self::Kutu,
        Self::Koli,
        Self::Ton,
    ];

    /// Arayüzde gösterilen Türkçe ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Adet => "Adet",
            Self::Kilogram => "Kilogram (kg)",
            Self::Gram => "Gram (g)",
            Self::Litre => "Litre",
            Self::Millilitre => "Mililitre (ml)",
            Self::Metre => "Metre",
            Self::Metrekare => "Metrekare (m²)",
            Self::Paket => "Paket",
            Self::Kutu => "Kutu",
            Self::Koli => "Koli",
            Self::Ton => "Ton",
        }
    }

    /// Stok gösteriminde kullanılan kısa etiket (örneğin `"kg"`).
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Adet => "adet",
            Self::Kilogram => "kg",
            Self::Gram => "g",
            Self::Litre => "litre",
            Self::Millilitre => "ml",
            Self::Metre => "metre",
            Self::Metrekare => "m²",
            Self::Paket => "paket",
            Self::Kutu => "kutu",
            Self::Koli => "koli",
            Self::Ton => "ton",
        }
    }

    /// Veritabanında saklanan değer (şemadaki `CHECK` ile aynı olmalıdır).
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Adet => "adet",
            Self::Kilogram => "kg",
            Self::Gram => "g",
            Self::Litre => "litre",
            Self::Millilitre => "ml",
            Self::Metre => "metre",
            Self::Metrekare => "m2",
            Self::Paket => "paket",
            Self::Kutu => "kutu",
            Self::Koli => "koli",
            Self::Ton => "ton",
        }
    }

    /// Veritabanındaki değerden birimi çözer.
    pub fn from_db(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|unit| unit.as_db() == value)
    }
}

/// Veritabanından okunmuş ürün kaydı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    /// Birincil anahtar (fatura satırları bu kimliğe bağlanacaktır).
    pub id: i64,
    /// Benzersiz ürün kodu.
    pub code: String,
    /// Barkod (isteğe bağlı, girilmişse benzersiz).
    pub barcode: Option<String>,
    /// Ürün adı.
    pub name: String,
    /// Ölçü birimi.
    pub unit: Unit,
    /// Kategori (serbest metin).
    pub category: Option<String>,
    /// Alış fiyatı (kuruş).
    pub purchase_price: Money,
    /// Satış fiyatı (kuruş).
    pub sale_price: Money,
    /// KDV oranı (0-100).
    pub vat_rate: i64,
    /// Minimum stok (mili-birim).
    pub min_stock: Quantity,
    /// Güncel stok (mili-birim; hareketlerle güncellenir).
    pub current_stock: Quantity,
    /// Notlar.
    pub notes: Option<String>,
    /// Kayıt aktif mi? (pasif kayıtlar silinmez, listede kalır)
    pub active: bool,
    /// Oluşturulma zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub created_at: String,
    /// Son güncellenme zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub updated_at: String,
}

impl Product {
    /// Durum etiketi: `Aktif` / `Pasif`.
    pub fn status_label(&self) -> &'static str {
        if self.active { "Aktif" } else { "Pasif" }
    }

    /// Düşük stok durumu: minimum tanımlı ve güncel stok minimumun altında.
    pub fn is_low_stock(&self) -> bool {
        self.min_stock.is_positive() && self.current_stock < self.min_stock
    }

    /// Güncel stok, birim etiketiyle (`"1,5 kg"`).
    pub fn formatted_stock(&self) -> String {
        self.current_stock
            .format_with_label(self.unit.short_label())
    }

    /// Minimum stok, birim etiketiyle (`"10 adet"`).
    pub fn formatted_min_stock(&self) -> String {
        self.min_stock.format_with_label(self.unit.short_label())
    }

    /// KDV oranı etiketi (`"%20"`).
    pub fn vat_label(&self) -> String {
        format!("%{}", self.vat_rate)
    }

    /// Kayıt arama ölçütüne uyuyor mu?
    ///
    /// `needle` değeri [`crate::customer::normalize_for_search`] ile normalize
    /// edilmiş olmalıdır. Ürün kodu, barkod, ad ve kategori aranır.
    pub fn matches_search(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }

        let haystacks = [
            Some(self.code.as_str()),
            self.barcode.as_deref(),
            Some(self.name.as_str()),
            self.category.as_deref(),
        ];

        haystacks
            .into_iter()
            .flatten()
            .any(|value| crate::customer::normalize_for_search(value).contains(needle))
    }
}

/// Stok hareketinin türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MovementType {
    /// Ürün kartı oluşturulurken girilen başlangıç stoğu.
    Initial,
    /// Stok girişi (stoğu artırır).
    #[default]
    In,
    /// Stok çıkışı (stoğu azaltır).
    Out,
    /// Düzeltme (işaret kullanıcı tarafından seçilir).
    Adjust,
    /// Alış faturasından kaynaklanan otomatik stok girişi.
    PurchaseInvoice,
    /// Satış faturasından kaynaklanan otomatik stok çıkışı.
    SalesInvoice,
}

impl MovementType {
    /// Olası tüm hareket türleri (veritabanı şemasıyla aynı sırada).
    pub const ALL: [Self; 6] = [
        Self::Initial,
        Self::In,
        Self::Out,
        Self::Adjust,
        Self::PurchaseInvoice,
        Self::SalesInvoice,
    ];

    /// Yeni hareket formunda seçilebilen türler.
    ///
    /// `İlk Stok` yalnızca ürün kartı oluşturulurken, fatura türleri ise
    /// yalnızca fatura kesilirken üretilir; elle hareket formunda sunulmaz.
    pub const FORM_OPTIONS: [Self; 3] = [Self::In, Self::Out, Self::Adjust];

    /// Arayüzde gösterilen Türkçe ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Initial => "İlk Stok",
            Self::In => "Giriş",
            Self::Out => "Çıkış",
            Self::Adjust => "Düzeltme",
            Self::PurchaseInvoice => "Alış Faturası",
            Self::SalesInvoice => "Satış Faturası",
        }
    }

    /// Veritabanında saklanan değer.
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Initial => "ilk_stok",
            Self::In => "giris",
            Self::Out => "cikis",
            Self::Adjust => "duzeltme",
            Self::PurchaseInvoice => "alis_faturasi",
            Self::SalesInvoice => "satis_faturasi",
        }
    }

    /// Veritabanındaki değerden türü çözer.
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "ilk_stok" => Some(Self::Initial),
            "giris" => Some(Self::In),
            "cikis" => Some(Self::Out),
            "duzeltme" => Some(Self::Adjust),
            "alis_faturasi" => Some(Self::PurchaseInvoice),
            "satis_faturasi" => Some(Self::SalesInvoice),
            _ => None,
        }
    }
}

/// Veritabanından okunmuş stok hareketi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockMovement {
    /// Birincil anahtar.
    pub id: i64,
    /// Hareketin bağlı olduğu ürünün kimliği.
    pub product_id: i64,
    /// Ürün kodu (liste gösterimi için birleştirilmiş alan).
    pub product_code: String,
    /// Ürün adı (liste gösterimi için birleştirilmiş alan).
    pub product_name: String,
    /// Ürünün ölçü birimi (miktar gösterimi için).
    pub product_unit: Unit,
    /// Hareket türü.
    pub movement_type: MovementType,
    /// İşaretli miktar değişimi (mili-birim).
    pub quantity_delta: Quantity,
    /// Hareket tarihi (`YYYY-MM-DD`).
    pub movement_date: String,
    /// Açıklama.
    pub description: Option<String>,
    /// Oluşturulma zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub created_at: String,
}

impl StockMovement {
    /// Miktar etiketi, ürünün birim kısaltmasıyla (`"+2,5 kg"`).
    pub fn formatted_quantity(&self) -> String {
        self.quantity_delta
            .format_signed_with_label(self.product_unit.short_label())
    }
}

/// Ürün listesi sorgu ölçütleri.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductFilter {
    /// Kod, barkod, ad veya kategoride aranacak metin.
    pub search: String,
    /// Yalnızca bu durumu göster (`None` = tümü).
    pub active: Option<bool>,
    /// Yalnızca düşük stoklu ürünleri göster.
    pub low_stock_only: bool,
}

impl ProductFilter {
    /// Ölçüt hiçbir şeyi kısıtlıyor mu?
    pub fn is_empty(&self) -> bool {
        self.search.trim().is_empty() && self.active.is_none() && !self.low_stock_only
    }
}

/// Arayüz formundaki ham kullanıcı girdisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDraft {
    /// Ürün kodu.
    pub code: String,
    /// Barkod.
    pub barcode: String,
    /// Ürün adı.
    pub name: String,
    /// Ölçü birimi.
    pub unit: Unit,
    /// Kategori.
    pub category: String,
    /// Alış fiyatı (serbest metin; `Money::parse` ile çözümlenir).
    pub purchase_price: String,
    /// Satış fiyatı.
    pub sale_price: String,
    /// KDV oranı (serbest metin; 0-100).
    pub vat_rate: String,
    /// Minimum stok (serbest metin; `Quantity::parse` ile çözümlenir).
    pub min_stock: String,
    /// Notlar.
    pub notes: String,
    /// Aktif/Pasif durumu.
    pub active: bool,
    /// Başlangıç stoğu (yalnızca yeni kayıtta kullanılır).
    pub starting_stock: String,
}

impl Default for ProductDraft {
    /// Yeni kayıt formunun başlangıç değerleri (yeni kayıt varsayılan olarak aktiftir).
    fn default() -> Self {
        Self {
            code: String::new(),
            barcode: String::new(),
            name: String::new(),
            unit: Unit::default(),
            category: String::new(),
            purchase_price: String::new(),
            sale_price: String::new(),
            vat_rate: "20".to_owned(),
            min_stock: String::new(),
            notes: String::new(),
            active: true,
            starting_stock: String::new(),
        }
    }
}

impl ProductDraft {
    /// Var olan bir kaydı düzenlemek üzere form verisine dönüştürür.
    pub fn from_product(product: &Product) -> Self {
        Self {
            code: product.code.clone(),
            barcode: product.barcode.clone().unwrap_or_default(),
            name: product.name.clone(),
            unit: product.unit,
            category: product.category.clone().unwrap_or_default(),
            purchase_price: product.purchase_price.format_turkish(),
            sale_price: product.sale_price.format_turkish(),
            vat_rate: product.vat_rate.to_string(),
            min_stock: product.min_stock.format_turkish(),
            notes: product.notes.clone().unwrap_or_default(),
            active: product.active,
            starting_stock: String::new(),
        }
    }

    /// Formdaki veriyi doğrular ve veritabanına yazılabilir kayda dönüştürür.
    ///
    /// Boş bırakılan isteğe bağlı alanlar `None` olur; metinler kırpılır.
    /// Birden fazla hata varsa **hepsi** birden döner. Negatif fiyat ve miktar
    /// kabul edilmez.
    pub fn validate(&self) -> Result<ValidatedProduct, FieldErrors> {
        let mut errors = FieldErrors::default();

        let code = self.code.trim().to_owned();
        if code.is_empty() {
            errors.push(Field::Code, "Ürün kodu zorunludur.");
        } else if code.chars().count() > CODE_MAX_LEN {
            errors.push(
                Field::Code,
                format!("Ürün kodu en fazla {CODE_MAX_LEN} karakter olabilir."),
            );
        } else if code.chars().any(char::is_whitespace) {
            errors.push(Field::Code, "Ürün kodu boşluk içeremez.");
        }

        let name = self.name.trim();
        if name.is_empty() {
            errors.push(Field::Name, "Ürün adı zorunludur.");
        } else if name.chars().count() > NAME_MAX_LEN {
            errors.push(
                Field::Name,
                format!("Ürün adı en fazla {NAME_MAX_LEN} karakter olabilir."),
            );
        }

        let barcode = trimmed_optional(&self.barcode);
        if let Some(barcode) = &barcode {
            let length = barcode.chars().count();
            if !(BARCODE_MIN_LEN..=BARCODE_MAX_LEN).contains(&length) {
                errors.push(
                    Field::Barcode,
                    format!(
                        "Barkod {BARCODE_MIN_LEN}-{BARCODE_MAX_LEN} karakter arasında olmalıdır."
                    ),
                );
            } else if barcode.chars().any(char::is_whitespace) {
                errors.push(Field::Barcode, "Barkod boşluk içeremez.");
            }
        }

        let category = limited_optional(
            &self.category,
            CATEGORY_MAX_LEN,
            Field::Category,
            "Kategori",
            &mut errors,
        );

        let purchase_price = money_field(
            &self.purchase_price,
            Field::PurchasePrice,
            "Alış fiyatı",
            &mut errors,
        );
        let sale_price = money_field(
            &self.sale_price,
            Field::SalePrice,
            "Satış fiyatı",
            &mut errors,
        );

        let vat_rate = match self.vat_rate.trim() {
            "" => {
                errors.push(Field::VatRate, "KDV oranı zorunludur.");
                0
            }
            text => match text.parse::<i64>() {
                Ok(rate) if (0..=100).contains(&rate) => rate,
                _ => {
                    errors.push(
                        Field::VatRate,
                        "KDV oranı 0 ile 100 arasında bir tam sayı olmalıdır.",
                    );
                    0
                }
            },
        };

        let min_stock = quantity_field(
            &self.min_stock,
            Field::MinStock,
            "Minimum stok",
            true,
            &mut errors,
        );
        let starting_stock = quantity_field(
            &self.starting_stock,
            Field::StartingStock,
            "Başlangıç stoğu",
            true,
            &mut errors,
        );

        let notes = limited_optional(
            &self.notes,
            NOTES_MAX_LEN,
            Field::Notes,
            "Notlar",
            &mut errors,
        );

        if errors.is_empty() {
            Ok(ValidatedProduct {
                code,
                barcode,
                name: name.to_owned(),
                unit: self.unit,
                category,
                purchase_price: purchase_price.unwrap_or(Money::ZERO),
                sale_price: sale_price.unwrap_or(Money::ZERO),
                vat_rate,
                min_stock: min_stock.unwrap_or(Quantity::ZERO),
                notes,
                active: self.active,
                starting_stock: starting_stock.unwrap_or(Quantity::ZERO),
            })
        } else {
            Err(errors)
        }
    }
}

/// Doğrulamadan geçmiş, veritabanına yazılabilir ürün kaydı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProduct {
    /// Ürün kodu (zorunlu).
    pub code: String,
    /// Barkod (isteğe bağlı).
    pub barcode: Option<String>,
    /// Ürün adı (zorunlu).
    pub name: String,
    /// Ölçü birimi.
    pub unit: Unit,
    /// Kategori.
    pub category: Option<String>,
    /// Alış fiyatı (kuruş).
    pub purchase_price: Money,
    /// Satış fiyatı (kuruş).
    pub sale_price: Money,
    /// KDV oranı (0-100).
    pub vat_rate: i64,
    /// Minimum stok (mili-birim).
    pub min_stock: Quantity,
    /// Notlar.
    pub notes: Option<String>,
    /// Aktif/Pasif durumu.
    pub active: bool,
    /// Başlangıç stoğu (yeni kayıtta `ilk_stok` hareketi üretir).
    pub starting_stock: Quantity,
}

/// Yeni hareket formundaki ham kullanıcı girdisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovementDraft {
    /// Hareket türü (formda İlk Stok sunulmaz).
    pub movement_type: MovementType,
    /// Kullanıcının girdiği pozitif miktar metni.
    pub quantity: String,
    /// Düzeltme türünde miktarın eksi yönlü olup olmadığı.
    pub adjustment_negative: bool,
    /// Hareket tarihi (`YYYY-MM-DD`).
    pub date: String,
    /// Açıklama (düzeltmede zorunlu).
    pub description: String,
}

impl Default for MovementDraft {
    fn default() -> Self {
        Self {
            movement_type: MovementType::In,
            quantity: String::new(),
            adjustment_negative: false,
            date: today_iso(),
            description: String::new(),
        }
    }
}

impl MovementDraft {
    /// Miktar alanı geçerli (pozitif) ise değerini döndürür.
    pub fn quantity_value(&self) -> Option<Quantity> {
        let quantity = Quantity::parse(&self.quantity).ok()?;
        quantity.is_positive().then_some(quantity)
    }

    /// Miktarın stok üzerindeki işaretli etkisi (geçersizse `None`).
    pub fn delta(&self) -> Option<Quantity> {
        let quantity = self.quantity_value()?;
        match self.movement_type {
            MovementType::Out => quantity.checked_neg().ok(),
            MovementType::Adjust if self.adjustment_negative => quantity.checked_neg().ok(),
            _ => Some(quantity),
        }
    }

    /// Formdaki veriyi doğrular ve veritabanına yazılabilir harekete dönüştürür.
    pub fn validate(&self) -> Result<ValidatedMovement, FieldErrors> {
        let mut errors = FieldErrors::default();

        match Quantity::parse(&self.quantity) {
            Ok(quantity) if quantity.is_positive() => {}
            Ok(quantity) if quantity.is_zero() => {
                errors.push(Field::MovementQuantity, "Miktar sıfır olamaz.");
            }
            Ok(_) => {
                errors.push(Field::MovementQuantity, "Miktar pozitif girilmelidir.");
            }
            Err(error) => {
                errors.push(Field::MovementQuantity, error.to_string());
            }
        }

        let date = self.date.trim();
        if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            errors.push(
                Field::MovementDate,
                "Tarih YYYY-AA-GG biçiminde ve geçerli olmalıdır (örnek: 2026-10-18).",
            );
        }

        let description = self.description.trim();
        if self.movement_type == MovementType::Adjust && description.is_empty() {
            errors.push(
                Field::MovementDescription,
                "Düzeltme hareketi için açıklama zorunludur.",
            );
        } else if description.chars().count() > MOVEMENT_DESCRIPTION_MAX_LEN {
            errors.push(
                Field::MovementDescription,
                format!("Açıklama en fazla {MOVEMENT_DESCRIPTION_MAX_LEN} karakter olabilir."),
            );
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let quantity_delta = self.delta().ok_or_else(|| {
            let mut errors = FieldErrors::default();
            errors.push(Field::MovementQuantity, "Miktar geçersiz.");
            errors
        })?;

        Ok(ValidatedMovement {
            movement_type: self.movement_type,
            quantity_delta,
            movement_date: date.to_owned(),
            description: trimmed_optional(description),
        })
    }
}

/// Doğrulamadan geçmiş, veritabanına yazılabilir stok hareketi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMovement {
    /// Hareket türü.
    pub movement_type: MovementType,
    /// İşaretli miktar değişimi (mili-birim; sıfır olamaz).
    pub quantity_delta: Quantity,
    /// Hareket tarihi (`YYYY-MM-DD`).
    pub movement_date: String,
    /// Açıklama.
    pub description: Option<String>,
}

/// Formda hata üretilebilen alanlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// Ürün kodu.
    Code,
    /// Barkod.
    Barcode,
    /// Ürün adı.
    Name,
    /// Kategori.
    Category,
    /// Alış fiyatı.
    PurchasePrice,
    /// Satış fiyatı.
    SalePrice,
    /// KDV oranı.
    VatRate,
    /// Minimum stok.
    MinStock,
    /// Notlar.
    Notes,
    /// Başlangıç stoğu.
    StartingStock,
    /// Stok hareketi miktarı.
    MovementQuantity,
    /// Stok hareketi tarihi.
    MovementDate,
    /// Stok hareketi açıklaması.
    MovementDescription,
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

    /// İlk hata mesajı (içe aktarma önizlemesinde kısa sebep göstermek için).
    pub fn first_message(&self) -> Option<&str> {
        self.entries.first().map(|(_, message)| message.as_str())
    }
}

/// Bugünün tarihini `YYYY-MM-DD` biçiminde döndürür (yerel saat).
pub fn today_iso() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
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

/// Fiyat alanını doğrular; negatif tutarlar reddedilir.
fn money_field(value: &str, field: Field, label: &str, errors: &mut FieldErrors) -> Option<Money> {
    if value.trim().is_empty() {
        return Some(Money::ZERO);
    }

    match Money::parse(value) {
        Ok(money) if money.is_negative() => {
            errors.push(field, format!("{label} negatif olamaz."));
            None
        }
        Ok(money) => Some(money),
        Err(_) => {
            errors.push(
                field,
                format!("{label} geçerli bir tutar olmalıdır (örnek: 1.234,56)."),
            );
            None
        }
    }
}

/// Miktar alanını doğrular; negatif miktarlar reddedilir veya sıfır kabul edilir.
fn quantity_field(
    value: &str,
    field: Field,
    label: &str,
    allow_zero: bool,
    errors: &mut FieldErrors,
) -> Option<Quantity> {
    if value.trim().is_empty() {
        return Some(Quantity::ZERO);
    }

    match Quantity::parse(value) {
        Ok(quantity) if quantity.is_negative() => {
            errors.push(field, format!("{label} negatif olamaz."));
            None
        }
        Ok(quantity) if quantity.is_zero() && !allow_zero => {
            errors.push(field, format!("{label} sıfırdan büyük olmalıdır."));
            None
        }
        Ok(quantity) => Some(quantity),
        Err(_) => {
            errors.push(
                field,
                format!("{label} geçerli bir miktar olmalıdır (örnek: 10 veya 1,5)."),
            );
            None
        }
    }
}

/// Var olan kodlara bakarak kullanılmamış bir ürün kodu önerir.
///
/// `URN-` önekli kodlardaki en büyük sayının bir fazlası önerilir; bu kod
/// kullanımdaysa ilk boş numara bulunur. Kod benzersizliğinin asıl güvencesi
/// veritabanındaki `UNIQUE` kısıtıdır; bu fonksiyon yalnızca kolaylık sağlar.
pub fn suggest_next_code(existing_codes: &[String]) -> String {
    let used: HashSet<String> = existing_codes
        .iter()
        .map(|code| crate::customer::normalize_for_search(code))
        .collect();

    let highest = existing_codes
        .iter()
        .filter_map(|code| code.strip_prefix(CODE_PREFIX))
        .filter_map(|suffix| suffix.parse::<u32>().ok())
        .max()
        .unwrap_or(0);

    let first = highest.saturating_add(1);
    for offset in 0..100_000_u32 {
        let candidate = format!("{CODE_PREFIX}{:04}", first.saturating_add(offset));
        if !used.contains(&crate::customer::normalize_for_search(&candidate)) {
            return candidate;
        }
    }

    // Pratikte erişilemez: 100.000 boş numara bulunamazsa son aday kullanılır.
    format!("{CODE_PREFIX}{first:04}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> ProductDraft {
        ProductDraft {
            code: "URN-0001".to_owned(),
            name: "Vida 4x40".to_owned(),
            ..ProductDraft::default()
        }
    }

    fn ornek_product() -> Product {
        Product {
            id: 3,
            code: "URN-0003".to_owned(),
            barcode: Some("8691234567890".to_owned()),
            name: "Çelik Vida".to_owned(),
            unit: Unit::Kilogram,
            category: Some("Bağlantı Elemanları".to_owned()),
            purchase_price: Money::from_kurus(1234),
            sale_price: Money::from_kurus(2000),
            vat_rate: 20,
            min_stock: Quantity::from_milli(10_000),
            current_stock: Quantity::from_milli(5_000),
            notes: Some("Not".to_owned()),
            active: true,
            created_at: "2026-10-18 10:00:00".to_owned(),
            updated_at: "2026-10-18 10:00:00".to_owned(),
        }
    }

    #[test]
    fn birim_etiketleri_ve_veritabani_karsiligi_tutarlidir() {
        for unit in Unit::ALL {
            assert!(!unit.label().is_empty());
            assert!(!unit.short_label().is_empty());
            assert_eq!(
                Unit::from_db(unit.as_db()),
                Some(unit),
                "{unit:?} veritabanından geri okunabilmeli"
            );
        }
        assert_eq!(Unit::from_db("bilinmeyen"), None);
    }

    #[test]
    fn hareket_turu_etiketleri_ve_veritabani_karsiligi_tutarlidir() {
        for movement_type in MovementType::ALL {
            assert!(!movement_type.label().is_empty());
            assert_eq!(
                MovementType::from_db(movement_type.as_db()),
                Some(movement_type)
            );
        }
        assert_eq!(MovementType::from_db("satis"), None);
        assert_eq!(MovementType::FORM_OPTIONS.len(), 3);
    }

    #[test]
    fn fatura_hareket_turleri_yeni_yonde_tanimlidir() {
        assert_eq!(MovementType::PurchaseInvoice.as_db(), "alis_faturasi");
        assert_eq!(MovementType::SalesInvoice.as_db(), "satis_faturasi");
        assert_eq!(
            MovementType::from_db("alis_faturasi"),
            Some(MovementType::PurchaseInvoice)
        );
        assert_eq!(
            MovementType::from_db("satis_faturasi"),
            Some(MovementType::SalesInvoice)
        );
        // Fatura türleri elle hareket formunda sunulmaz.
        assert!(!MovementType::FORM_OPTIONS.contains(&MovementType::PurchaseInvoice));
        assert!(!MovementType::FORM_OPTIONS.contains(&MovementType::SalesInvoice));
    }

    #[test]
    fn gecerli_form_verisi_dogrulamadan_gecer() {
        let form = ProductDraft {
            code: "URN-0001".to_owned(),
            barcode: " 8691234567890 ".to_owned(),
            name: " Vida 4x40 ".to_owned(),
            unit: Unit::Kilogram,
            category: "Bağlantı".to_owned(),
            purchase_price: "1.234,56".to_owned(),
            sale_price: "2.000".to_owned(),
            vat_rate: "20".to_owned(),
            min_stock: "10".to_owned(),
            notes: "  ".to_owned(),
            active: true,
            starting_stock: "1,5".to_owned(),
        };

        let validated = form.validate().expect("doğrulama geçmeli");

        assert_eq!(validated.code, "URN-0001");
        assert_eq!(validated.name, "Vida 4x40");
        assert_eq!(validated.barcode.as_deref(), Some("8691234567890"));
        assert_eq!(validated.unit, Unit::Kilogram);
        assert_eq!(validated.purchase_price.kurus(), 123_456);
        assert_eq!(validated.sale_price.kurus(), 200_000);
        assert_eq!(validated.vat_rate, 20);
        assert_eq!(validated.min_stock.milli(), 10_000);
        assert_eq!(validated.starting_stock.milli(), 1500);
        assert_eq!(validated.notes, None, "boş alan None olmalı");
        assert!(validated.active);
    }

    #[test]
    fn zorunlu_alanlar_bos_birakilamaz() {
        let form = ProductDraft {
            code: "   ".to_owned(),
            name: String::new(),
            ..ProductDraft::default()
        };
        let errors = form.validate().expect_err("hatalar bekleniyordu");
        assert!(errors.get(Field::Code).is_some());
        assert!(errors.get(Field::Name).is_some());
    }

    #[test]
    fn kod_bosluk_icermez_ve_uzunluk_sinirlidir() {
        let mut form = draft();
        form.code = "URN 0001".to_owned();
        assert!(form.validate().is_err());

        form.code = "K".repeat(CODE_MAX_LEN + 1);
        let errors = form.validate().unwrap_err();
        assert!(errors.get(Field::Code).unwrap().contains("en fazla"));
    }

    #[test]
    fn barkod_uzunlugu_kontrol_edilir() {
        let mut form = draft();

        form.barcode = "123".to_owned();
        assert!(form.validate().is_err(), "kısa barkod reddedilmeli");

        form.barcode = "K".repeat(BARCODE_MAX_LEN + 1);
        assert!(form.validate().is_err(), "uzun barkod reddedilmeli");

        form.barcode = "1234".to_owned();
        assert!(form.validate().is_ok(), "4 karakterli barkod geçerli");

        form.barcode = String::new();
        assert!(form.validate().is_ok(), "boş barkod serbest olmalı");
    }

    #[test]
    fn fiyat_ve_miktar_dogrulamasi_negatifi_reddeder() {
        let mut form = draft();

        form.purchase_price = "-5".to_owned();
        assert!(form.validate().is_err());

        form.purchase_price = "abc".to_owned();
        assert!(form.validate().is_err());

        form.purchase_price = String::new();
        form.min_stock = "-1".to_owned();
        assert!(form.validate().is_err());

        form.min_stock = "1,5".to_owned();
        form.starting_stock = "-2".to_owned();
        assert!(form.validate().is_err());
    }

    #[test]
    fn kdv_orani_sinirlari_kontrol_edilir() {
        let mut form = draft();

        for rate in ["0", "1", "8", "10", "18", "20", "100"] {
            form.vat_rate = rate.to_owned();
            assert!(form.validate().is_ok(), "%{rate} geçerli olmalı");
        }

        for rate in ["101", "-1", "abc", ""] {
            form.vat_rate = rate.to_owned();
            assert!(form.validate().is_err(), "%{rate} geçersiz olmalı");
        }
    }

    #[test]
    fn tum_hatalar_tek_seferde_doner() {
        let form = ProductDraft {
            code: "GEÇERSİZ KOD".to_owned(),
            name: String::new(),
            purchase_price: "abc".to_owned(),
            sale_price: "-1".to_owned(),
            vat_rate: "200".to_owned(),
            min_stock: "-2".to_owned(),
            barcode: "12".to_owned(),
            ..ProductDraft::default()
        };

        let errors = form.validate().expect_err("hatalar bekleniyordu");
        for field in [
            Field::Code,
            Field::Name,
            Field::Barcode,
            Field::PurchasePrice,
            Field::SalePrice,
            Field::VatRate,
            Field::MinStock,
        ] {
            assert!(errors.get(field).is_some(), "{field:?} hatası bekleniyordu");
        }
        assert!(errors.get(Field::Notes).is_none());
    }

    #[test]
    fn kayittan_form_verisine_donus_kayipsizdir() {
        let product = ornek_product();
        let form = ProductDraft::from_product(&product);
        let validated = form.validate().expect("doğrulama geçmeli");

        assert_eq!(validated.code, product.code);
        assert_eq!(validated.barcode, product.barcode);
        assert_eq!(validated.name, product.name);
        assert_eq!(validated.unit, product.unit);
        assert_eq!(validated.category, product.category);
        assert_eq!(validated.purchase_price, product.purchase_price);
        assert_eq!(validated.sale_price, product.sale_price);
        assert_eq!(validated.vat_rate, product.vat_rate);
        assert_eq!(validated.min_stock, product.min_stock);
        assert_eq!(validated.notes, product.notes);
        assert_eq!(validated.active, product.active);
        assert!(
            validated.starting_stock.is_zero(),
            "düzenlemede başlangıç stoğu boş olmalı"
        );
    }

    #[test]
    fn dusuk_stok_ve_bicimlendirme_dogru_calisir() {
        let mut product = ornek_product();

        // 5 kg stok, 10 kg minimum -> düşük stok.
        assert!(product.is_low_stock());
        assert_eq!(product.formatted_stock(), "5 kg");
        assert_eq!(product.formatted_min_stock(), "10 kg");
        assert_eq!(product.vat_label(), "%20");
        assert_eq!(product.status_label(), "Aktif");

        product.current_stock = Quantity::from_milli(15_000);
        assert!(
            !product.is_low_stock(),
            "minimum üstünde düşük stok sayılmaz"
        );

        product.min_stock = Quantity::ZERO;
        product.current_stock = Quantity::ZERO;
        assert!(
            !product.is_low_stock(),
            "minimum yoksa düşük stok uyarısı yok"
        );
    }

    #[test]
    fn hareket_miktari_isareti_ture_ve_secime_gore_verilir() {
        let mut draft = MovementDraft {
            quantity: "5".to_owned(),
            ..MovementDraft::default()
        };

        draft.movement_type = MovementType::In;
        assert_eq!(draft.delta().unwrap().milli(), 5000);

        draft.movement_type = MovementType::Out;
        assert_eq!(draft.delta().unwrap().milli(), -5000);

        draft.movement_type = MovementType::Adjust;
        draft.adjustment_negative = true;
        assert_eq!(draft.delta().unwrap().milli(), -5000);

        draft.adjustment_negative = false;
        assert_eq!(draft.delta().unwrap().milli(), 5000);

        draft.quantity = "0".to_owned();
        assert!(draft.delta().is_none(), "sıfır miktarın etkisi yok");
    }

    #[test]
    fn hareket_dogrulamasi_duzeltmede_aciklama_ister() {
        let mut draft = MovementDraft {
            quantity: "3".to_owned(),
            date: "2026-10-18".to_owned(),
            ..MovementDraft::default()
        };

        draft.movement_type = MovementType::In;
        assert!(draft.validate().is_ok(), "girişte açıklama serbest");

        draft.movement_type = MovementType::Adjust;
        let errors = draft.validate().expect_err("düzeltmede açıklama zorunlu");
        assert!(errors.get(Field::MovementDescription).is_some());

        draft.description = "Sayım düzeltmesi".to_owned();
        let validated = draft.validate().unwrap();
        assert_eq!(validated.quantity_delta.milli(), 3000);
        assert_eq!(validated.description.as_deref(), Some("Sayım düzeltmesi"));
    }

    #[test]
    fn hareket_dogrulamasi_miktar_ve_tarihi_kontrol_eder() {
        let mut draft = MovementDraft {
            quantity: "-1".to_owned(),
            date: "18.10.2026".to_owned(),
            ..MovementDraft::default()
        };

        let errors = draft.validate().expect_err("geçersiz girdi");
        assert!(errors.get(Field::MovementQuantity).is_some());
        assert!(errors.get(Field::MovementDate).is_some());

        draft.quantity = "1,5".to_owned();
        draft.date = "2026-10-18".to_owned();
        let validated = draft.validate().unwrap();
        assert_eq!(validated.quantity_delta.milli(), 1500);
        assert_eq!(validated.movement_date, "2026-10-18");
    }

    #[test]
    fn kod_onerisi_bos_ve_dolu_listelerde_calisir() {
        assert_eq!(suggest_next_code(&[]), "URN-0001");

        let codes = vec![
            "URN-0001".to_owned(),
            "URN-0003".to_owned(),
            "URU-9".to_owned(),
        ];
        assert_eq!(suggest_next_code(&codes), "URN-0004");

        let codes = vec!["urn-0001".to_owned()];
        assert_eq!(suggest_next_code(&codes), "URN-0002");
    }

    #[test]
    fn arama_metni_turkce_karakterlerden_etkilenmez() {
        let product = ornek_product();

        for needle in ["ÇELİK", "celik", "çelik", "VİDA", "vida"] {
            assert!(
                product.matches_search(&crate::customer::normalize_for_search(needle)),
                "'{needle}' araması eşleşmeliydi"
            );
        }

        assert!(product.matches_search(&crate::customer::normalize_for_search("URN-0003")));
        assert!(product.matches_search(&crate::customer::normalize_for_search("8691234567890")));
        assert!(product.matches_search(&crate::customer::normalize_for_search("baglanti")));
        assert!(product.matches_search(""));
        assert!(!product.matches_search(&crate::customer::normalize_for_search("bulunmayan")));
    }

    #[test]
    fn filtre_bos_mu_bilgisi_dogrudur() {
        assert!(ProductFilter::default().is_empty());

        let filter = ProductFilter {
            search: "  ".to_owned(),
            ..ProductFilter::default()
        };
        assert!(filter.is_empty(), "boş arama filtre sayılmaz");

        let filter = ProductFilter {
            low_stock_only: true,
            ..ProductFilter::default()
        };
        assert!(!filter.is_empty());

        let filter = ProductFilter {
            active: Some(false),
            ..ProductFilter::default()
        };
        assert!(!filter.is_empty());
    }
}
