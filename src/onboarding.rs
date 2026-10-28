// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! İlk çalıştırma sihirbazının saf veri modeli.
//!
//! Şirket bilgileri `app_settings` tablosunda `company_*` anahtarlarıyla
//! saklanır. Sihirbazın veritabanına yazdığı tüm kayıtlar tek bir transaction
//! içinde tamamlanır (bkz. [`crate::db::Database::apply_onboarding`]).

use crate::cash::ValidatedCashAccount;
use crate::db::Database;
use crate::error::Result;
use crate::income_expense::{EntryType, ValidatedCategory};

/// `app_settings` içinde şirket bilgilerini tutan anahtarlar.
pub const COMPANY_NAME_KEY: &str = "company_name";
/// Şirket yetkilisi.
pub const COMPANY_CONTACT_KEY: &str = "company_contact";
/// Şirket telefonu.
pub const COMPANY_PHONE_KEY: &str = "company_phone";
/// Şirket e-postası.
pub const COMPANY_EMAIL_KEY: &str = "company_email";
/// Şirket adresi.
pub const COMPANY_ADDRESS_KEY: &str = "company_address";
/// Vergi dairesi.
pub const COMPANY_TAX_OFFICE_KEY: &str = "company_tax_office";
/// Vergi numarası.
pub const COMPANY_TAX_NUMBER_KEY: &str = "company_tax_number";
/// Sihirbazın tamamlanıp tamamlanmadığı (`0` / `1`).
pub const ONBOARDING_COMPLETED_KEY: &str = "onboarding_completed";

/// Sihirbazda toplanan şirket bilgileri.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompanyInfo {
    /// Ticari unvan.
    pub name: String,
    /// Yetkili kişi.
    pub contact_person: String,
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
}

impl CompanyInfo {
    /// Veritabanındaki `company_*` anahtarlarından bilgileri okur.
    pub fn load(database: &Database) -> Result<Self> {
        let get =
            |key: &str| -> Result<String> { Ok(database.setting_get(key)?.unwrap_or_default()) };
        Ok(Self {
            name: get(COMPANY_NAME_KEY)?,
            contact_person: get(COMPANY_CONTACT_KEY)?,
            phone: get(COMPANY_PHONE_KEY)?,
            email: get(COMPANY_EMAIL_KEY)?,
            address: get(COMPANY_ADDRESS_KEY)?,
            tax_office: get(COMPANY_TAX_OFFICE_KEY)?,
            tax_number: get(COMPANY_TAX_NUMBER_KEY)?,
        })
    }

    /// Transaction içinde yazılacak anahtar/değer çiftleri.
    pub fn as_pairs(&self) -> [(&'static str, &str); 7] {
        [
            (COMPANY_NAME_KEY, self.name.as_str()),
            (COMPANY_CONTACT_KEY, self.contact_person.as_str()),
            (COMPANY_PHONE_KEY, self.phone.as_str()),
            (COMPANY_EMAIL_KEY, self.email.as_str()),
            (COMPANY_ADDRESS_KEY, self.address.as_str()),
            (COMPANY_TAX_OFFICE_KEY, self.tax_office.as_str()),
            (COMPANY_TAX_NUMBER_KEY, self.tax_number.as_str()),
        ]
    }

    /// Özet adımında gösterilecek kısa unvan.
    pub fn display_name(&self) -> &str {
        let trimmed = self.name.trim();
        if trimmed.is_empty() {
            "Şirket bilgisi girilmedi"
        } else {
            trimmed
        }
    }
}

/// Sihirbazın dört adımı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnboardingStep {
    /// Şirket bilgileri.
    #[default]
    Company,
    /// İlk kasa/banka hesabı.
    Cash,
    /// Kategori paketleri.
    Categories,
    /// Özet ve tamamlama.
    Summary,
}

impl OnboardingStep {
    /// Tüm adımlar (ileri/geri gezinme ve testler için).
    pub const ALL: [Self; 4] = [Self::Company, Self::Cash, Self::Categories, Self::Summary];

    /// Adım numarası (1 tabanlı).
    pub fn number(self) -> usize {
        match self {
            Self::Company => 1,
            Self::Cash => 2,
            Self::Categories => 3,
            Self::Summary => 4,
        }
    }

    /// Adım başlığı.
    pub fn title(self) -> &'static str {
        match self {
            Self::Company => "Şirket bilgileri",
            Self::Cash => "İlk kasa/banka hesabı",
            Self::Categories => "Kategori paketleri",
            Self::Summary => "Özet",
        }
    }

    /// Bir sonraki adım (`Summary` son adımdır).
    pub fn next(self) -> Self {
        match self {
            Self::Company => Self::Cash,
            Self::Cash => Self::Categories,
            Self::Categories => Self::Summary,
            Self::Summary => Self::Summary,
        }
    }

    /// Bir önceki adım (`Company` ilk adımdır).
    pub fn previous(self) -> Self {
        match self {
            Self::Company => Self::Company,
            Self::Cash => Self::Company,
            Self::Categories => Self::Cash,
            Self::Summary => Self::Categories,
        }
    }
}

/// Sihirbazda önerilen hazır kategori.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryPackage {
    /// Kategori adı.
    pub name: &'static str,
    /// Gelir/gider türü.
    pub category_type: EntryType,
}

/// Varsayılan kategori paketleri.
pub const CATEGORY_PACKAGES: [CategoryPackage; 14] = [
    CategoryPackage {
        name: "Satış geliri",
        category_type: EntryType::Gelir,
    },
    CategoryPackage {
        name: "Hizmet geliri",
        category_type: EntryType::Gelir,
    },
    CategoryPackage {
        name: "Diğer gelirler",
        category_type: EntryType::Gelir,
    },
    CategoryPackage {
        name: "Kira",
        category_type: EntryType::Gider,
    },
    CategoryPackage {
        name: "Personel",
        category_type: EntryType::Gider,
    },
    CategoryPackage {
        name: "Elektrik",
        category_type: EntryType::Gider,
    },
    CategoryPackage {
        name: "Su",
        category_type: EntryType::Gider,
    },
    CategoryPackage {
        name: "Doğalgaz",
        category_type: EntryType::Gider,
    },
    CategoryPackage {
        name: "İnternet / telefon",
        category_type: EntryType::Gider,
    },
    CategoryPackage {
        name: "Kargo",
        category_type: EntryType::Gider,
    },
    CategoryPackage {
        name: "Vergi ve harçlar",
        category_type: EntryType::Gider,
    },
    CategoryPackage {
        name: "Bakım ve onarım",
        category_type: EntryType::Gider,
    },
    CategoryPackage {
        name: "Ofis malzemeleri",
        category_type: EntryType::Gider,
    },
    CategoryPackage {
        name: "Diğer giderler",
        category_type: EntryType::Gider,
    },
];

/// Sihirbaz tamamlandığında tek transaction içinde yazılacak plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OnboardingPlan {
    /// Şirket bilgileri.
    pub company: CompanyInfo,
    /// İsteğe bağlı ilk kasa/banka hesabı.
    pub cash: Option<ValidatedCashAccount>,
    /// Seçilen hazır kategoriler.
    pub categories: Vec<ValidatedCategory>,
}
