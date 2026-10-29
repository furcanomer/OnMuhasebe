// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Cari hesap iş modeli ve doğrulama kuralları.
//!
//! Bu modül **saf iş mantığıdır**: veritabanına ve arayüze bağımlılığı yoktur,
//! bu nedenle birim testleriyle eksiksiz doğrulanabilir.
//!
//! Üç ayrı tip vardır ve aralarındaki akış bilinçlidir:
//!
//! | Tip                  | Nerede kullanılır                                  |
//! |----------------------|----------------------------------------------------|
//! | [`CustomerDraft`]    | Arayüz formundaki ham (doğrulanmamış) kullanıcı girdisi |
//! | [`ValidatedCustomer`]| Doğrulamadan geçmiş, veritabanına yazılabilir kayıt   |
//! | [`Customer`]         | Veritabanından okunmuş, kimliği olan kayıt           |
//!
//! Böylece geçersiz veri SQL katmanına **hiç ulaşmaz**: veritabanı yalnızca
//! [`ValidatedCustomer`] kabul eder.
//!
//! ## Bakiye hakkında
//!
//! Bakiye **gerçek verilerden** hesaplanır: kesinleşmiş satış faturalarının
//! toplamından kesinleşmiş alış faturalarının toplamı çıkarılır. İptal edilen
//! faturalar hesaba katılmaz.
//!
//! * **Pozitif** bakiye: cari bize borçlu (bizden mal/hizmet aldı).
//! * **Negatif** bakiye: biz cariye borçluyuz (ondan mal/hizmet aldık).
//!
//! Bakiye, fatura ve tahsilat/ödeme hareketlerinin net toplamıdır ve yalnızca
//! [`Customer::balance`] içinde hesaplanır; arayüz ve veritabanı şeması bu
//! hesaptan bağımsızdır.

use std::collections::HashSet;

use crate::money::Money;

/// Otomatik üretilen cari kodu öneki (örnek: `CAR-0001`).
pub const CODE_PREFIX: &str = "CAR-";

/// Cari kodunun en fazla uzunluğu (karakter).
pub const CODE_MAX_LEN: usize = 32;
/// Ad / firma unvanının en fazla uzunluğu.
pub const NAME_MAX_LEN: usize = 200;
/// Telefonun en fazla uzunluğu.
pub const PHONE_MAX_LEN: usize = 30;
/// E-posta adresinin en fazla uzunluğu.
pub const EMAIL_MAX_LEN: usize = 254;
/// Vergi dairesi adının en fazla uzunluğu.
pub const TAX_OFFICE_MAX_LEN: usize = 100;
/// Yetkili kişi adının en fazla uzunluğu.
pub const CONTACT_MAX_LEN: usize = 100;
/// Adresin en fazla uzunluğu.
pub const ADDRESS_MAX_LEN: usize = 500;
/// Notların en fazla uzunluğu.
pub const NOTES_MAX_LEN: usize = 1000;

/// Cari hesabın türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CustomerType {
    /// Yalnızca müşteri.
    #[default]
    Customer,
    /// Yalnızca tedarikçi.
    Supplier,
    /// Hem müşteri hem tedarikçi.
    Both,
}

impl CustomerType {
    /// Seçim listesinde gösterilecek sıradaki tüm türler.
    pub const ALL: [Self; 3] = [Self::Customer, Self::Supplier, Self::Both];

    /// Arayüzde gösterilen Türkçe ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Customer => "Müşteri",
            Self::Supplier => "Tedarikçi",
            Self::Both => "Her İkisi",
        }
    }

    /// Veritabanında saklanan değer.
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Customer => "musteri",
            Self::Supplier => "tedarikci",
            Self::Both => "ikisi",
        }
    }

    /// Veritabanındaki değerden türü çözer.
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "musteri" => Some(Self::Customer),
            "tedarikci" => Some(Self::Supplier),
            "ikisi" => Some(Self::Both),
            _ => None,
        }
    }
}

/// Veritabanından okunmuş cari hesap kaydı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Customer {
    /// Birincil anahtar (ileride hareketler bu kimliğe bağlanacak).
    pub id: i64,
    /// Benzersiz cari kodu.
    pub code: String,
    /// Ad veya firma unvanı.
    pub name: String,
    /// Cari türü.
    pub kind: CustomerType,
    /// Telefon (metin; baştaki sıfırlar korunur).
    pub phone: Option<String>,
    /// E-posta.
    pub email: Option<String>,
    /// Adres.
    pub address: Option<String>,
    /// Vergi dairesi.
    pub tax_office: Option<String>,
    /// Vergi numarası.
    pub tax_number: Option<String>,
    /// T.C. kimlik numarası.
    pub national_id: Option<String>,
    /// Yetkili kişi.
    pub contact_person: Option<String>,
    /// Notlar.
    pub notes: Option<String>,
    /// Kayıt aktif mi? (pasif kayıtlar silinmez, listede kalır)
    pub active: bool,
    /// Oluşturulma zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub created_at: String,
    /// Son güncellenme zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub updated_at: String,
    /// Kesinleşmiş satış faturalarının toplamı (KDV dâhil, kuruş).
    ///
    /// İptal edilen faturalar katılmaz. [`Customer::balance`] bu alanı kullanır.
    pub sales_total: Money,
    /// Kesinleşmiş alış faturalarının toplamı (KDV dâhil, kuruş).
    ///
    /// İptal edilen faturalar katılmaz. [`Customer::balance`] bu alanı kullanır.
    pub purchase_total: Money,
    /// Bu cariye bağlı fatura sayısı (iptaller dâhil).
    ///
    /// [`Customer::has_movements`] bu alanı kullanır.
    pub invoice_count: usize,
    /// Kesinleşmiş tahsilatların toplamı (kuruş).
    ///
    /// İptal edilen tahsilatlar katılmaz. [`Customer::balance`] bu alanı kullanır.
    pub collection_total: Money,
    /// Kesinleşmiş ödemelerin toplamı (kuruş).
    ///
    /// İptal edilen ödemeler katılmaz. [`Customer::balance`] bu alanı kullanır.
    pub payment_total: Money,
    /// Bu cariye bağlı **kesinleşmiş** tahsilat/ödeme kaydı sayısı.
    ///
    /// İptal edilen kayıtlar katılmaz. [`Customer::has_movements`] bu alanı
    /// kullanır.
    pub payment_count: usize,
}

impl Customer {
    /// Güncel bakiye: kesinleşmiş satışlar eksi alışlar, tahsilatlar düşülür,
    /// ödemeler eklenir.
    ///
    /// `bakiye = kesin satış - kesin alış - kesin tahsilat + kesin ödeme`
    ///
    /// * **Pozitif**: cari bize borçlu.
    /// * **Negatif**: biz cariye borçluyuz.
    ///
    /// İptal edilen faturalar ve iptal edilen tahsilat/ödemeler hesaba katılmaz.
    pub fn balance(&self) -> Money {
        Money::from_kurus(
            self.sales_total
                .kurus()
                .saturating_sub(self.purchase_total.kurus())
                .saturating_sub(self.collection_total.kurus())
                .saturating_add(self.payment_total.kurus()),
        )
    }

    /// Durum etiketi: `Aktif` / `Pasif`.
    pub fn status_label(&self) -> &'static str {
        if self.active { "Aktif" } else { "Pasif" }
    }

    /// Bu cariye bağlı herhangi bir fatura veya **kesinleşmiş** tahsilat/ödeme
    /// kaydı var mı?
    ///
    /// Fatura sayısı iptalleri de kapsar; iptal edilmiş bir tahsilat/ödeme ise
    /// tek başına hareket oluşturmaz.
    pub fn has_movements(&self) -> bool {
        self.invoice_count > 0 || self.payment_count > 0
    }

    /// Vergi kimlik bilgisi: `("VKN", vergi numarası)` veya
    /// `("TCKN", T.C. kimlik numarası)`.
    pub fn tax_identifier(&self) -> Option<(&'static str, &str)> {
        match (&self.tax_number, &self.national_id) {
            (Some(tax_number), _) => Some(("VKN", tax_number)),
            (None, Some(national_id)) => Some(("TCKN", national_id)),
            (None, None) => None,
        }
    }

    /// Kayıt arama ölçütüne uyuyor mu?
    ///
    /// `needle` değeri [`normalize_for_search`] ile normalize edilmiş olmalıdır.
    /// Cari kodu, ad, telefon, vergi numarası ve T.C. kimlik numarası aranır.
    pub fn matches_search(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }

        let haystacks = [
            Some(self.code.as_str()),
            Some(self.name.as_str()),
            self.phone.as_deref(),
            self.tax_number.as_deref(),
            self.national_id.as_deref(),
        ];

        haystacks
            .into_iter()
            .flatten()
            .any(|value| normalize_for_search(value).contains(needle))
    }
}

/// Cari hesap listesi sorgu ölçütleri.
///
/// `kind` ve `active` alanları `None` ise ilgili ölçüt uygulanmaz.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomerFilter {
    /// Cari kodu, ad, telefon veya vergi kimlik numarasında aranacak metin.
    pub search: String,
    /// Yalnızca bu türü göster (`None` = tümü).
    pub kind: Option<CustomerType>,
    /// Yalnızca bu durumu göster (`None` = tümü).
    pub active: Option<bool>,
}

impl CustomerFilter {
    /// Ölçüt hiçbir şeyi kısıtlıyor mu?
    pub fn is_empty(&self) -> bool {
        self.search.trim().is_empty() && self.kind.is_none() && self.active.is_none()
    }
}

/// Arayüz formundaki ham kullanıcı girdisi.
///
/// Tüm alanlar metindir; `validate` çağrılana kadar veritabanına yazılamaz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerDraft {
    /// Cari kodu.
    pub code: String,
    /// Ad veya firma unvanı.
    pub name: String,
    /// Cari türü.
    pub kind: CustomerType,
    /// Telefon.
    pub phone: String,
    /// E-posta.
    pub email: String,
    /// Adres.
    pub address: String,
    /// Vergi dairesi.
    pub tax_office: String,
    /// Vergi numarası.
    pub tax_number: String,
    /// T.C. kimlik numarası.
    pub national_id: String,
    /// Yetkili kişi.
    pub contact_person: String,
    /// Notlar.
    pub notes: String,
    /// Aktif/Pasif durumu.
    pub active: bool,
}

impl Default for CustomerDraft {
    /// Yeni kayıt formunun başlangıç değerleri (yeni kayıt varsayılan olarak aktiftir).
    fn default() -> Self {
        Self {
            code: String::new(),
            name: String::new(),
            kind: CustomerType::default(),
            phone: String::new(),
            email: String::new(),
            address: String::new(),
            tax_office: String::new(),
            tax_number: String::new(),
            national_id: String::new(),
            contact_person: String::new(),
            notes: String::new(),
            active: true,
        }
    }
}

impl CustomerDraft {
    /// Var olan bir kaydı düzenlemek üzere form verisine dönüştürür.
    pub fn from_customer(customer: &Customer) -> Self {
        Self {
            code: customer.code.clone(),
            name: customer.name.clone(),
            kind: customer.kind,
            phone: customer.phone.clone().unwrap_or_default(),
            email: customer.email.clone().unwrap_or_default(),
            address: customer.address.clone().unwrap_or_default(),
            tax_office: customer.tax_office.clone().unwrap_or_default(),
            tax_number: customer.tax_number.clone().unwrap_or_default(),
            national_id: customer.national_id.clone().unwrap_or_default(),
            contact_person: customer.contact_person.clone().unwrap_or_default(),
            notes: customer.notes.clone().unwrap_or_default(),
            active: customer.active,
        }
    }

    /// Formdaki veriyi doğrular ve veritabanına yazılabilir kayda dönüştürür.
    ///
    /// Boş bırakılan isteğe bağlı alanlar `None` olur; metinler kırpılır.
    /// Birden fazla hata varsa **hepsi** birden döner ki kullanıcı formu tek
    /// seferde düzeltebilsin.
    pub fn validate(&self) -> Result<ValidatedCustomer, FieldErrors> {
        let mut errors = FieldErrors::default();

        let code = self.code.trim().to_owned();
        if code.is_empty() {
            errors.push(Field::Code, "Cari kodu zorunludur.");
        } else if code.chars().count() > CODE_MAX_LEN {
            errors.push(
                Field::Code,
                format!("Cari kodu en fazla {CODE_MAX_LEN} karakter olabilir."),
            );
        } else if code.chars().any(char::is_whitespace) {
            errors.push(Field::Code, "Cari kodu boşluk içeremez.");
        }

        let name = self.name.trim();
        if name.is_empty() {
            errors.push(Field::Name, "Ad veya firma unvanı zorunludur.");
        } else if name.chars().count() > NAME_MAX_LEN {
            errors.push(
                Field::Name,
                format!("Ad en fazla {NAME_MAX_LEN} karakter olabilir."),
            );
        }

        let phone = trimmed_optional(&self.phone);
        if let Some(phone) = &phone
            && let Some(message) = phone_error(phone)
        {
            errors.push(Field::Phone, message);
        }

        let email = trimmed_optional(&self.email);
        if let Some(email) = &email
            && let Some(message) = email_error(email)
        {
            errors.push(Field::Email, message);
        }

        let tax_office = limited_optional(
            &self.tax_office,
            TAX_OFFICE_MAX_LEN,
            Field::TaxOffice,
            "Vergi dairesi",
            &mut errors,
        );

        let tax_number = trimmed_optional(&self.tax_number);
        if let Some(tax_number) = &tax_number
            && let Some(message) = digits_error(tax_number, &[10, 11], "Vergi numarası")
        {
            errors.push(Field::TaxNumber, message);
        }

        let national_id = trimmed_optional(&self.national_id);
        if let Some(national_id) = &national_id
            && let Some(message) = digits_error(national_id, &[11], "T.C. kimlik numarası")
        {
            errors.push(Field::NationalId, message);
        }

        let contact_person = limited_optional(
            &self.contact_person,
            CONTACT_MAX_LEN,
            Field::ContactPerson,
            "Yetkili kişi",
            &mut errors,
        );

        let address = limited_optional(
            &self.address,
            ADDRESS_MAX_LEN,
            Field::Address,
            "Adres",
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
            Ok(ValidatedCustomer {
                code,
                name: name.to_owned(),
                kind: self.kind,
                phone,
                email,
                address,
                tax_office,
                tax_number,
                national_id,
                contact_person,
                notes,
                active: self.active,
            })
        } else {
            Err(errors)
        }
    }
}

/// Doğrulamadan geçmiş, veritabanına yazılabilir cari hesap kaydı.
///
/// Tüm metin alanları kırpılmıştır ve boş alanlar `None` olmuştur. Bu tipin
/// dışında hiçbir veri SQL katmanına geçemez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCustomer {
    /// Cari kodu (zorunlu).
    pub code: String,
    /// Ad veya firma unvanı (zorunlu).
    pub name: String,
    /// Cari türü.
    pub kind: CustomerType,
    /// Telefon.
    pub phone: Option<String>,
    /// E-posta.
    pub email: Option<String>,
    /// Adres.
    pub address: Option<String>,
    /// Vergi dairesi.
    pub tax_office: Option<String>,
    /// Vergi numarası.
    pub tax_number: Option<String>,
    /// T.C. kimlik numarası.
    pub national_id: Option<String>,
    /// Yetkili kişi.
    pub contact_person: Option<String>,
    /// Notlar.
    pub notes: Option<String>,
    /// Aktif/Pasif durumu.
    pub active: bool,
}

/// Formda hata üretilebilen alanlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// Cari kodu.
    Code,
    /// Ad / unvan.
    Name,
    /// Telefon.
    Phone,
    /// E-posta.
    Email,
    /// Adres.
    Address,
    /// Vergi dairesi.
    TaxOffice,
    /// Vergi numarası.
    TaxNumber,
    /// T.C. kimlik numarası.
    NationalId,
    /// Yetkili kişi.
    ContactPerson,
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

    /// İlk hata mesajı (içe aktarma önizlemesinde kısa sebep göstermek için).
    pub fn first_message(&self) -> Option<&str> {
        self.entries.first().map(|(_, message)| message.as_str())
    }
}

/// Metni kırpar; boş kalırsa `None` döner.
pub fn trimmed_optional(value: &str) -> Option<String> {
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

/// Telefon numarası doğrulaması; hata varsa mesaj döner.
///
/// Numara **sayıya çevrilmez**: baştaki sıfır ve ayraçlar korunur.
fn phone_error(phone: &str) -> Option<String> {
    if phone.chars().count() > PHONE_MAX_LEN {
        return Some(format!(
            "Telefon en fazla {PHONE_MAX_LEN} karakter olabilir."
        ));
    }

    let allowed = |character: char| {
        character.is_ascii_digit() || matches!(character, ' ' | '+' | '-' | '(' | ')' | '.' | '/')
    };
    if !phone.chars().all(allowed) {
        return Some("Telefon yalnızca rakam, boşluk ve + - ( ) . / içerebilir.".to_owned());
    }

    if !phone.starts_with(|character: char| {
        character.is_ascii_digit() || character == '+' || character == '('
    }) {
        return Some("Telefon rakam, + veya ( ile başlamalıdır.".to_owned());
    }

    let digit_count = phone.chars().filter(char::is_ascii_digit).count();
    if !(7..=15).contains(&digit_count) {
        return Some("Telefon 7 ile 15 rakam arasında olmalıdır.".to_owned());
    }

    None
}

/// E-posta biçim doğrulaması; hata varsa mesaj döner.
///
/// Amaç tam RFC doğrulaması değil, yaygın yazım hatalarını yakalamaktır; adres
/// geçerliliği son kertede kullanıcının sorumluluğundadır.
fn email_error(email: &str) -> Option<String> {
    if email.chars().count() > EMAIL_MAX_LEN {
        return Some(format!(
            "E-posta en fazla {EMAIL_MAX_LEN} karakter olabilir."
        ));
    }
    if email.chars().any(char::is_whitespace) {
        return Some("E-posta boşluk içeremez.".to_owned());
    }

    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return Some("E-posta yalnızca bir adet @ içermelidir.".to_owned());
    }

    let invalid = "Geçerli bir e-posta adresi girin (örnek: ad@firma.com).";

    if local.is_empty() || local.chars().count() > 64 {
        return Some(invalid.to_owned());
    }
    if local.starts_with('.') || local.ends_with('.') || local.contains("..") {
        return Some(invalid.to_owned());
    }
    if !local
        .chars()
        .all(|character| character.is_alphanumeric() || "!#$%&'*+/=?^_`{|}~.-".contains(character))
    {
        return Some(invalid.to_owned());
    }

    if domain.is_empty()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.starts_with('-')
        || domain.ends_with('.')
        || domain.ends_with('-')
        || domain.contains("..")
    {
        return Some(invalid.to_owned());
    }

    let labels: Vec<&str> = domain.split('.').collect();
    if labels
        .iter()
        .any(|label| label.is_empty() || !label.chars().all(|c| c.is_alphanumeric() || c == '-'))
    {
        return Some(invalid.to_owned());
    }

    let top_level = labels.last().copied().unwrap_or_default();
    if top_level.chars().count() < 2 || !top_level.chars().all(char::is_alphabetic) {
        return Some(invalid.to_owned());
    }

    None
}

/// Yalnızca rakamlardan oluşan ve belirtilen uzunluklardan birinde olan bir
/// numarayı doğrular; hata varsa mesaj döner.
fn digits_error(value: &str, allowed_lengths: &[usize], label: &str) -> Option<String> {
    if !value.chars().all(|character| character.is_ascii_digit()) {
        return Some(format!("{label} yalnızca rakamlardan oluşmalıdır."));
    }

    if !allowed_lengths.contains(&value.chars().count()) {
        let lengths = allowed_lengths
            .iter()
            .map(|length| length.to_string())
            .collect::<Vec<_>>()
            .join(" veya ");
        return Some(format!("{label} {lengths} haneli olmalıdır."));
    }

    None
}

/// Kullanıcının yazdığı metni arama için karşılaştırılabilir biçime getirir.
///
/// Türkçe karakterler sadeleştirilir (`İstanbul` -> `istanbul`, `ç` -> `c`) ve
/// küçük harfe çevrilir. Böylece arama, Türkçe büyük/küçük harf ve aksan
/// farklarından etkilenmez; SQLite'ın yalnızca ASCII üzerinde çalışan
/// `LIKE`/`NOCASE` davranışına bağımlı kalınmaz.
pub fn normalize_for_search(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    for character in text.chars() {
        let mapped = match character {
            'İ' | 'I' | 'ı' | 'i' => 'i',
            'Ş' | 'ş' => 's',
            'Ğ' | 'ğ' => 'g',
            'Ü' | 'ü' => 'u',
            'Ö' | 'ö' => 'o',
            'Ç' | 'ç' => 'c',
            'Â' | 'â' => 'a',
            'Î' | 'î' => 'i',
            'Û' | 'û' => 'u',
            other => other,
        };
        for lowered in mapped.to_lowercase() {
            normalized.push(lowered);
        }
    }
    normalized
}

/// Var olan kodlara bakarak kullanılmamış bir cari kodu önerir.
///
/// `CAR-` önekli kodlardaki en büyük sayının bir fazlası önerilir; bu kod
/// kullanımdaysa ilk boş numara bulunur. Kod benzersizliğinin asıl güvencesi
/// veritabanındaki `UNIQUE` kısıtıdır; bu fonksiyon yalnızca kolaylık sağlar.
pub fn suggest_next_code(existing_codes: &[String]) -> String {
    let used: HashSet<String> = existing_codes
        .iter()
        .map(|code| normalize_for_search(code))
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
        if !used.contains(&normalize_for_search(&candidate)) {
            return candidate;
        }
    }

    // Pratikte erişilemez: 100.000 boş numara bulunamazsa son aday kullanılır.
    format!("{CODE_PREFIX}{first:04}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> CustomerDraft {
        CustomerDraft {
            code: "CAR-0001".to_owned(),
            name: "ABC Ticaret Ltd. Şti.".to_owned(),
            ..CustomerDraft::default()
        }
    }

    fn ornek_customer() -> Customer {
        Customer {
            id: 7,
            code: "CAR-0007".to_owned(),
            name: "Öztürk İnşaat".to_owned(),
            kind: CustomerType::Both,
            phone: Some("0212 555 12 34".to_owned()),
            email: Some("info@ozturk.com".to_owned()),
            address: Some("Ankara".to_owned()),
            tax_office: Some("Çankaya".to_owned()),
            tax_number: Some("1234567890".to_owned()),
            national_id: None,
            contact_person: Some("Ayşe Öztürk".to_owned()),
            notes: Some("Not".to_owned()),
            active: true,
            created_at: "2026-10-18 10:00:00".to_owned(),
            updated_at: "2026-10-18 10:00:00".to_owned(),
            sales_total: Money::ZERO,
            purchase_total: Money::ZERO,
            invoice_count: 0,
            collection_total: Money::ZERO,
            payment_total: Money::ZERO,
            payment_count: 0,
        }
    }

    #[test]
    fn cari_turu_etiketleri_ve_veritabani_karsiligi_tutarlidir() {
        for kind in CustomerType::ALL {
            assert!(!kind.label().is_empty());
            assert_eq!(
                CustomerType::from_db(kind.as_db()),
                Some(kind),
                "{kind:?} veritabanından geri okunabilmeli"
            );
        }
        assert_eq!(CustomerType::from_db("bilinmeyen"), None);
    }

    #[test]
    fn gecerli_form_verisi_dogrulamadan_gecer() {
        let mut form = draft();
        form.phone = " 0212 555 12 34 ".to_owned();
        form.email = " info@abc.com ".to_owned();
        form.address = "   ".to_owned();
        form.tax_number = "1234567890".to_owned();
        form.national_id = "12345678901".to_owned();

        let validated = form.validate().expect("doğrulama geçmeli");

        assert_eq!(validated.code, "CAR-0001");
        assert_eq!(validated.name, "ABC Ticaret Ltd. Şti.");
        assert_eq!(validated.phone.as_deref(), Some("0212 555 12 34"));
        assert_eq!(validated.email.as_deref(), Some("info@abc.com"));
        assert_eq!(
            validated.address, None,
            "boş alan veritabanına None olarak yazılmalı"
        );
        assert!(validated.active);
    }

    #[test]
    fn bos_telefon_bastaki_sifiri_korur() {
        let mut form = draft();
        form.phone = "0212 555 12 34".to_owned();
        let validated = form.validate().unwrap();
        assert_eq!(validated.phone.as_deref(), Some("0212 555 12 34"));
        assert!(validated.phone.unwrap().starts_with('0'));
    }

    #[test]
    fn zorunlu_alanlar_bos_birakilamaz() {
        let form = CustomerDraft {
            code: "   ".to_owned(),
            name: String::new(),
            ..CustomerDraft::default()
        };

        let errors = form.validate().expect_err("hatalar bekleniyordu");
        assert!(errors.get(Field::Code).is_some());
        assert!(errors.get(Field::Name).is_some());
        assert_eq!(errors.len(), 2);
        assert!(!errors.is_empty());
    }

    #[test]
    fn cari_kodu_bosluk_icermez_ve_uzunluk_sinirlidir() {
        let mut form = draft();
        form.code = "CAR 0001".to_owned();
        assert!(form.validate().is_err());

        form.code = "K".repeat(CODE_MAX_LEN + 1);
        let errors = form.validate().unwrap_err();
        assert!(errors.get(Field::Code).unwrap().contains("en fazla"));
    }

    #[test]
    fn gecersiz_telefon_reddedilir() {
        let mut form = draft();
        for phone in [
            "12345",
            "abc",
            "0212 555 12 34 555 6 6 6 6 6",
            "+",
            "05-55-abc",
        ] {
            form.phone = phone.to_owned();
            assert!(
                form.validate().is_err(),
                "'{phone}' geçersiz telefon sayılmalıydı"
            );
        }

        for phone in [
            "0212 555 12 34",
            "+90 (212) 555 12 34",
            "5551234567",
            "0212.555.12.34",
        ] {
            form.phone = phone.to_owned();
            assert!(
                form.validate().is_ok(),
                "'{phone}' geçerli telefon sayılmalıydı"
            );
        }
    }

    #[test]
    fn e_posta_bicimi_kontrol_edilir() {
        let mut form = draft();
        form.email = "info@firma.com".to_owned();
        assert!(form.validate().is_ok(), "geçerli e-posta kabul edilmeli");

        for email in [
            "",
            "info",
            "info@",
            "@firma.com",
            "info@firma",
            "info@@firma.com",
            "info @firma.com",
            "info@firma..com",
            ".info@firma.com",
            "info@firma.com.",
        ] {
            form.email = email.to_owned();
            if email.is_empty() {
                assert!(form.validate().is_ok(), "boş e-posta serbest olmalı");
            } else {
                assert!(
                    form.validate().is_err(),
                    "'{email}' geçersiz e-posta sayılmalıydı"
                );
            }
        }
    }

    #[test]
    fn vergi_ve_kimlik_numaralari_rakam_ve_hane_kontrolu_yapar() {
        let mut form = draft();

        form.tax_number = "1234567890".to_owned();
        assert!(form.validate().is_ok());

        form.tax_number = "12345678901".to_owned();
        assert!(form.validate().is_ok());

        form.tax_number = "123456789".to_owned();
        assert!(form.validate().is_err());

        form.tax_number = "123456789a".to_owned();
        assert!(form.validate().is_err());

        form.tax_number = String::new();
        form.national_id = "12345678901".to_owned();
        assert!(form.validate().is_ok());

        form.national_id = "1234567890".to_owned();
        assert!(form.validate().is_err());
    }

    #[test]
    fn tum_hatalar_tek_seferde_doner() {
        let form = CustomerDraft {
            code: "GEÇERSİZ KOD".to_owned(),
            name: String::new(),
            email: "bozuk".to_owned(),
            phone: "abc".to_owned(),
            tax_number: "12".to_owned(),
            ..CustomerDraft::default()
        };

        let errors = form.validate().expect_err("hatalar bekleniyordu");
        assert_eq!(errors.len(), 5);
        for field in [
            Field::Code,
            Field::Name,
            Field::Email,
            Field::Phone,
            Field::TaxNumber,
        ] {
            assert!(errors.get(field).is_some(), "{field:?} hatası bekleniyordu");
        }
        assert!(errors.get(Field::Notes).is_none());
    }

    #[test]
    fn hata_mesaji_duzeltilince_silinebilir() {
        let form = CustomerDraft {
            name: String::new(),
            ..draft()
        };
        let mut errors = form.validate().unwrap_err();
        assert_eq!(errors.len(), 1);

        errors.clear(Field::Name);
        assert!(errors.is_empty());
        assert_eq!(errors.get(Field::Name), None);
    }

    #[test]
    fn kayittan_form_verisine_donus_kayipsizdir() {
        let customer = ornek_customer();
        let form = CustomerDraft::from_customer(&customer);
        let validated = form.validate().expect("doğrulama geçmeli");

        assert_eq!(validated.code, customer.code);
        assert_eq!(validated.name, customer.name);
        assert_eq!(validated.kind, customer.kind);
        assert_eq!(validated.phone, customer.phone);
        assert_eq!(validated.email, customer.email);
        assert_eq!(validated.address, customer.address);
        assert_eq!(validated.tax_office, customer.tax_office);
        assert_eq!(validated.tax_number, customer.tax_number);
        assert_eq!(validated.national_id, customer.national_id);
        assert_eq!(validated.contact_person, customer.contact_person);
        assert_eq!(validated.notes, customer.notes);
        assert_eq!(validated.active, customer.active);
    }

    #[test]
    fn faturasiz_carinin_bakiyesi_sifirdir() {
        let customer = ornek_customer();
        assert!(customer.balance().is_zero());
        assert_eq!(customer.balance().format_with_symbol(), "0,00 ₺");
        assert!(!customer.has_movements());
    }

    #[test]
    fn bakiye_satis_eksi_alis_olarak_hesaplanir() {
        let mut customer = ornek_customer();

        // Satış 1.500,00 ₺, alış 400,00 ₺ -> cari bize 1.100,00 ₺ borçlu.
        customer.sales_total = Money::from_kurus(150_000);
        customer.purchase_total = Money::from_kurus(40_000);
        customer.invoice_count = 2;
        assert_eq!(customer.balance().kurus(), 110_000);
        assert_eq!(customer.balance().format_with_symbol(), "1.100,00 ₺");
        assert!(customer.has_movements());

        // Yalnızca alış: bakiye negatif (biz cariye borçluyuz).
        customer.sales_total = Money::ZERO;
        assert_eq!(customer.balance().kurus(), -40_000);
        assert!(customer.balance().is_negative());
    }

    #[test]
    fn bakiye_tahsilat_ve_odemeyi_de_hesaba_katar() {
        let mut customer = ornek_customer();

        // Satış 1.500 ₺, alış 400 ₺, tahsilat 500 ₺, ödeme 100 ₺
        // -> 150000 - 40000 - 50000 + 10000 = 70000 kuruş.
        customer.sales_total = Money::from_kurus(150_000);
        customer.purchase_total = Money::from_kurus(40_000);
        customer.collection_total = Money::from_kurus(50_000);
        customer.payment_total = Money::from_kurus(10_000);
        customer.invoice_count = 2;
        customer.payment_count = 2;
        assert_eq!(customer.balance().kurus(), 70_000);
        assert!(customer.has_movements());

        // Yalnızca tahsilat/ödeme kaydı olan cari de hareketlidir.
        let mut only_payments = ornek_customer();
        only_payments.payment_count = 1;
        assert!(only_payments.has_movements());
    }

    #[test]
    fn durum_ve_vergi_kimlik_etiketleri_dogrudur() {
        let mut customer = ornek_customer();
        assert_eq!(customer.status_label(), "Aktif");
        customer.active = false;
        assert_eq!(customer.status_label(), "Pasif");

        assert_eq!(customer.tax_identifier(), Some(("VKN", "1234567890")));
        customer.tax_number = None;
        customer.national_id = Some("12345678901".to_owned());
        assert_eq!(customer.tax_identifier(), Some(("TCKN", "12345678901")));
        customer.national_id = None;
        assert_eq!(customer.tax_identifier(), None);
    }

    #[test]
    fn kod_onerisi_bos_ve_dolu_listelerde_calisir() {
        assert_eq!(suggest_next_code(&[]), "CAR-0001");

        let codes = vec![
            "CAR-0001".to_owned(),
            "CAR-0003".to_owned(),
            "MUS-9".to_owned(),
        ];
        assert_eq!(suggest_next_code(&codes), "CAR-0004");

        // En büyük numara doluysa ilk boş numara bulunur.
        let codes = vec!["CAR-0001".to_owned(), "CAR-0002".to_owned()];
        assert_eq!(suggest_next_code(&codes), "CAR-0003");

        // Sayısal olmayan kodlar yok sayılır.
        let codes = vec!["CAR-XYZ".to_owned()];
        assert_eq!(suggest_next_code(&codes), "CAR-0001");

        // Büyük/küçük harf farkı olan kod da kullanımda sayılır.
        let codes = vec!["car-0001".to_owned()];
        assert_eq!(suggest_next_code(&codes), "CAR-0002");
    }

    #[test]
    fn arama_metni_turkce_karakterlerden_etkilenmez() {
        let customer = ornek_customer();

        for needle in ["ÖZTÜRK", "ozturk", "öztürk", "OZTURK"] {
            assert!(
                customer.matches_search(&normalize_for_search(needle)),
                "'{needle}' araması eşleşmeliydi"
            );
        }

        assert!(customer.matches_search(&normalize_for_search("CAR-0007")));
        assert!(customer.matches_search(&normalize_for_search("car-0007")));
        assert!(customer.matches_search(&normalize_for_search("0212")));
        assert!(customer.matches_search(&normalize_for_search("555 12 34")));
        assert!(customer.matches_search(&normalize_for_search("1234567890")));
        assert!(customer.matches_search(""));
        assert!(!customer.matches_search(&normalize_for_search("bulunmayan")));
    }

    #[test]
    fn arama_normalizasyonu_turkce_harfleri_sadelestirir() {
        assert_eq!(normalize_for_search("İstanbul"), "istanbul");
        assert_eq!(normalize_for_search("IĞDIR"), "igdir");
        assert_eq!(normalize_for_search("ÇĞİÖŞÜ"), "cgiosu");
        assert_eq!(normalize_for_search("  ABC  "), "  abc  ");
    }

    #[test]
    fn filtre_bos_mu_bilgisi_dogrudur() {
        assert!(CustomerFilter::default().is_empty());

        let filter = CustomerFilter {
            search: "  ".to_owned(),
            kind: None,
            active: None,
        };
        assert!(
            filter.is_empty(),
            "yalnızca boşluk içeren arama filtre sayılmaz"
        );

        let filter = CustomerFilter {
            search: "abc".to_owned(),
            ..CustomerFilter::default()
        };
        assert!(!filter.is_empty());

        let filter = CustomerFilter {
            kind: Some(CustomerType::Supplier),
            ..CustomerFilter::default()
        };
        assert!(!filter.is_empty());

        let filter = CustomerFilter {
            active: Some(false),
            ..CustomerFilter::default()
        };
        assert!(!filter.is_empty());
    }
}
