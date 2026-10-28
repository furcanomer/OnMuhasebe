// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Cari hesap veri erişimi.
//!
//! SQL ifadeleri yalnızca bu modülde bulunur; arayüz katmanı [`Database`]
//! üzerindeki metotları çağırır ve hiçbir zaman SQL yazmaz.
//!
//! ## Güvenlik ve bütünlük
//!
//! * Tüm kullanıcı girdileri **parametreli** sorgularla (`?1`, `?2`, ...)
//!   iletilir; sorgulara metin birleştirme (string concatenation) ile
//!   yerleştirilmez.
//! * Yazma metotları yalnızca [`ValidatedCustomer`] kabul eder; doğrulanmamış
//!   veri bu katmana gelemez.
//! * `code` alanındaki `UNIQUE` kısıtı veritabanı düzeyinde korunur; ihlal
//!   edilirse kullanıcıya anlaşılır bir hata ([`AppError::CustomerCodeTaken`])
//!   döner.
//! * Kayıt **silinmez**; pasife alma için [`Database::customer_set_active`]
//!   kullanılır.

use rusqlite::{OptionalExtension, Row, params};

use crate::customer::{
    Customer, CustomerFilter, CustomerType, ValidatedCustomer, suggest_next_code,
};
use crate::error::{AppError, Result};
use crate::money::Money;

use super::Database;

/// Cari hesap sorgularında kullanılan kolon listesi (sırası [`row_to_customer`] ile aynıdır).
///
/// Son altı kolon, fatura ve tahsilat/ödeme tablolarından **hesaplanan**
/// alanlardır: kesinleşmiş satış toplamı, kesinleşmiş alış toplamı, (iptaller
/// dâhil) fatura sayısı, kesinleşmiş tahsilat toplamı, kesinleşmiş ödeme
/// toplamı ve **kesinleşmiş** tahsilat/ödeme sayısı. Bu sayede bakiye ve
/// hareket bilgisi tek bir sorguda okunur.
const CUSTOMER_BASE_COLUMNS: &str = "c.id, c.code, c.name, c.kind, c.phone, c.email, c.address, \
                                      c.tax_office, c.tax_number, c.national_id, c.contact_person, \
                                      c.notes, c.active, c.created_at, c.updated_at";

/// Tek cari okunurken kullanılan bağıntılı toplamlar.
const CUSTOMER_COMPUTED_COLUMNS: &str = "COALESCE((SELECT SUM(i.grand_total) FROM invoices i \
                                            WHERE i.customer_id = c.id AND i.invoice_type = 'satis' \
                                              AND i.status = 'kesin'), 0), \
                                        COALESCE((SELECT SUM(i.grand_total) FROM invoices i \
                                            WHERE i.customer_id = c.id AND i.invoice_type = 'alis' \
                                              AND i.status = 'kesin'), 0), \
                                        (SELECT COUNT(*) FROM invoices i WHERE i.customer_id = c.id), \
                                        COALESCE((SELECT SUM(p.amount) FROM payments p \
                                            WHERE p.customer_id = c.id AND p.payment_type = 'tahsilat' \
                                              AND p.status = 'kesin'), 0), \
                                        COALESCE((SELECT SUM(p.amount) FROM payments p \
                                            WHERE p.customer_id = c.id AND p.payment_type = 'odeme' \
                                              AND p.status = 'kesin'), 0), \
                                        (SELECT COUNT(*) FROM payments p \
                                            WHERE p.customer_id = c.id AND p.status = 'kesin')";

/// Cari hesap türünün veritabanı değerini çözerken kullanılan hata mesajı.
const INVALID_KIND_INDEX: usize = 3;

impl Database {
    /// Ölçütlere uyan cari hesapları cari koduna göre sıralı döndürür.
    ///
    /// Tür ve durum ölçütleri SQL'de (parametreli olarak) uygulanır. Serbest
    /// metin araması ise Türkçe karakterlerden etkilenmemesi için Rust
    /// tarafında yapılır (bkz. [`crate::customer::normalize_for_search`]).
    pub fn customer_list(&self, filter: &CustomerFilter) -> Result<Vec<Customer>> {
        // Bağıntılı alt sorgular cari başına dört ayrı indeks taraması yapıyordu.
        // Toplamları bir kez hesaplayıp LEFT JOIN ile birleştirmek, özellikle
        // binlerce cari ve hareket içeren listelerde belirgin biçimde hızlıdır.
        let mut statement = self.connection.prepare(&format!(
            "SELECT {CUSTOMER_BASE_COLUMNS},
                    COALESCE(sales.sales_total, 0),
                    COALESCE(purchases.purchase_total, 0),
                    COALESCE(invoice_counts.invoice_count, 0),
                    COALESCE(collections.collection_total, 0),
                    COALESCE(payments_total.payment_total, 0),
                    COALESCE(payments_total.payment_count, 0)
             FROM customers c
             LEFT JOIN (
                 SELECT customer_id, SUM(grand_total) AS sales_total
                 FROM invoices
                 WHERE invoice_type = 'satis' AND status = 'kesin'
                 GROUP BY customer_id
             ) sales ON sales.customer_id = c.id
             LEFT JOIN (
                 SELECT customer_id, SUM(grand_total) AS purchase_total
                 FROM invoices
                 WHERE invoice_type = 'alis' AND status = 'kesin'
                 GROUP BY customer_id
             ) purchases ON purchases.customer_id = c.id
             LEFT JOIN (
                 SELECT customer_id, COUNT(*) AS invoice_count
                 FROM invoices
                 GROUP BY customer_id
             ) invoice_counts ON invoice_counts.customer_id = c.id
             LEFT JOIN (
                 SELECT customer_id, SUM(amount) AS collection_total
                 FROM payments
                 WHERE payment_type = 'tahsilat' AND status = 'kesin'
                 GROUP BY customer_id
             ) collections ON collections.customer_id = c.id
             LEFT JOIN (
                 SELECT customer_id,
                        SUM(CASE WHEN payment_type = 'odeme' THEN amount ELSE 0 END)
                            AS payment_total,
                        COUNT(*) AS payment_count
                 FROM payments
                 WHERE status = 'kesin'
                 GROUP BY customer_id
             ) payments_total ON payments_total.customer_id = c.id
             WHERE (?1 IS NULL OR c.kind = ?1)
               AND (?2 IS NULL OR c.active = ?2)
             ORDER BY c.code COLLATE NOCASE"
        ))?;

        let kind = filter.kind.map(CustomerType::as_db);
        let active = filter.active.map(i64::from);

        let rows = statement.query_map(params![kind, active], row_to_customer)?;

        let mut customers = Vec::new();
        for row in rows {
            customers.push(row?);
        }

        let needle = crate::customer::normalize_for_search(filter.search.trim());
        if !needle.is_empty() {
            customers.retain(|customer| customer.matches_search(&needle));
        }

        Ok(customers)
    }

    /// Kimliğine göre tek bir cari hesap okur.
    pub fn customer_get(&self, id: i64) -> Result<Option<Customer>> {
        let customer = self
            .connection
            .query_row(
                &format!(
                    "SELECT {CUSTOMER_BASE_COLUMNS}, {CUSTOMER_COMPUTED_COLUMNS}
                     FROM customers c WHERE c.id = ?1"
                ),
                [id],
                row_to_customer,
            )
            .optional()?;
        Ok(customer)
    }

    /// Toplam cari hesap sayısı (pasif kayıtlar dâhil).
    pub fn customer_count(&self) -> Result<usize> {
        let count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0))?;
        Ok(count.max(0) as usize)
    }

    /// Cari kodu kullanımda mı?
    ///
    /// `exclude_id` verilirse bu kayıt dışındaki kayıtlar denetlenir (düzenleme
    /// sırasında kaydın kendi kodunun çakışma sayılmaması için). Karşılaştırma
    /// büyük/küçük harf duyarsızdır; veritabanındaki `UNIQUE` kısıtı ile aynı
    /// kuralı uygular.
    pub fn customer_code_exists(&self, code: &str, exclude_id: Option<i64>) -> Result<bool> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM customers
                 WHERE code = ?1 COLLATE NOCASE AND (?2 IS NULL OR id <> ?2)
             )",
            params![code, exclude_id],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    /// Kullanılmamış bir cari kodu önerir (`CAR-0001`, `CAR-0002`, ...).
    pub fn customer_next_code(&self) -> Result<String> {
        let mut statement = self.connection.prepare("SELECT code FROM customers")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;

        let mut codes = Vec::new();
        for row in rows {
            codes.push(row?);
        }

        Ok(suggest_next_code(&codes))
    }

    /// Yeni cari hesap ekler ve oluşturulan kaydın kimliğini döndürür.
    pub fn customer_insert(&self, customer: &ValidatedCustomer) -> Result<i64> {
        let result = self.connection.execute(
            "INSERT INTO customers
                 (code, name, kind, phone, email, address, tax_office, tax_number,
                  national_id, contact_person, notes, active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                customer.code,
                customer.name,
                customer.kind.as_db(),
                customer.phone.as_deref(),
                customer.email.as_deref(),
                customer.address.as_deref(),
                customer.tax_office.as_deref(),
                customer.tax_number.as_deref(),
                customer.national_id.as_deref(),
                customer.contact_person.as_deref(),
                customer.notes.as_deref(),
                i64::from(customer.active),
            ],
        );

        match result {
            Ok(_) => Ok(self.connection.last_insert_rowid()),
            Err(error) => Err(map_write_error(error, &customer.code)),
        }
    }

    /// Var olan cari hesabı günceller.
    ///
    /// `updated_at` alanı veritabanı saatine göre kendiliğinden yenilenir;
    /// `created_at` değişmez. Kayıt yoksa [`AppError::CustomerNotFound`] döner.
    pub fn customer_update(&self, id: i64, customer: &ValidatedCustomer) -> Result<()> {
        let current = self
            .customer_get(id)?
            .ok_or(AppError::CustomerNotFound(id))?;
        if !self.session_is_manager() && current.active != customer.active {
            tracing::warn!(user_id = id, "yetki reddedildi: cari durumu değiştirme");
            return Err(AppError::PermissionDenied {
                operation: "cari durumunu değiştirme".to_owned(),
            });
        }
        let affected = self
            .connection
            .execute(
                "UPDATE customers
                    SET code = ?2,
                        name = ?3,
                        kind = ?4,
                        phone = ?5,
                        email = ?6,
                        address = ?7,
                        tax_office = ?8,
                        tax_number = ?9,
                        national_id = ?10,
                        contact_person = ?11,
                        notes = ?12,
                        active = ?13,
                        updated_at = datetime('now')
                  WHERE id = ?1",
                params![
                    id,
                    customer.code,
                    customer.name,
                    customer.kind.as_db(),
                    customer.phone.as_deref(),
                    customer.email.as_deref(),
                    customer.address.as_deref(),
                    customer.tax_office.as_deref(),
                    customer.tax_number.as_deref(),
                    customer.national_id.as_deref(),
                    customer.contact_person.as_deref(),
                    customer.notes.as_deref(),
                    i64::from(customer.active),
                ],
            )
            .map_err(|error| map_write_error(error, &customer.code))?;

        if affected == 0 {
            return Err(AppError::CustomerNotFound(id));
        }

        Ok(())
    }

    /// Cari hesabı aktif veya pasif yapar.
    ///
    /// Kayıt silinmez; ileride bağlanacak hareketlerin geçmişi korunur.
    pub fn customer_set_active(&self, id: i64, active: bool) -> Result<()> {
        self.require_admin("cari pasife alma/aktifleştirme")?;
        let affected = self.connection.execute(
            "UPDATE customers
                SET active = ?2, updated_at = datetime('now')
              WHERE id = ?1",
            params![id, i64::from(active)],
        )?;

        if affected == 0 {
            return Err(AppError::CustomerNotFound(id));
        }

        Ok(())
    }
}

/// Bir veritabanı satırını [`Customer`] tipine dönüştürür.
fn row_to_customer(row: &Row<'_>) -> rusqlite::Result<Customer> {
    let kind_value: String = row.get(INVALID_KIND_INDEX)?;
    let kind = CustomerType::from_db(&kind_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            INVALID_KIND_INDEX,
            rusqlite::types::Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen cari türü: '{kind_value}'"
            ))),
        )
    })?;

    Ok(Customer {
        id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        kind,
        phone: row.get(4)?,
        email: row.get(5)?,
        address: row.get(6)?,
        tax_office: row.get(7)?,
        tax_number: row.get(8)?,
        national_id: row.get(9)?,
        contact_person: row.get(10)?,
        notes: row.get(11)?,
        active: row.get::<_, i64>(12)? != 0,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        sales_total: Money::from_kurus(row.get(15)?),
        purchase_total: Money::from_kurus(row.get(16)?),
        invoice_count: row.get::<_, i64>(17)?.max(0) as usize,
        collection_total: Money::from_kurus(row.get(18)?),
        payment_total: Money::from_kurus(row.get(19)?),
        payment_count: row.get::<_, i64>(20)?.max(0) as usize,
    })
}

/// Yazma hatalarını kullanıcıya anlamlı gelen hatalara dönüştürür.
///
/// Cari kodu benzersizlik ihlali ayrıca ele alınır; diğer kısıt ihlalleri ve
/// tüm diğer SQLite hataları genel veritabanı hatası olarak döner.
fn map_write_error(error: rusqlite::Error, code: &str) -> AppError {
    if let rusqlite::Error::SqliteFailure(inner, _) = &error
        && inner.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    {
        return AppError::CustomerCodeTaken(code.to_owned());
    }

    AppError::Database(crate::error::sqlite_error_to_turkish(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::customer::CustomerDraft;

    fn valid(overrides: impl FnOnce(&mut CustomerDraft)) -> ValidatedCustomer {
        let mut draft = CustomerDraft {
            code: "CAR-0001".to_owned(),
            name: "ABC Ticaret".to_owned(),
            ..CustomerDraft::default()
        };
        overrides(&mut draft);
        draft.validate().expect("test verisi geçerli olmalı")
    }

    fn database() -> Database {
        Database::open_in_memory().expect("bellek içi veritabanı açılmalı")
    }

    #[test]
    fn cari_hesap_eklenir_ve_okunur() {
        let database = database();

        let id = database
            .customer_insert(&valid(|draft| {
                draft.code = "CAR-0001".to_owned();
                draft.name = "Öztürk İnşaat".to_owned();
                draft.kind = CustomerType::Both;
                draft.phone = "0212 555 12 34".to_owned();
                draft.email = "info@ozturk.com".to_owned();
                draft.address = "Ankara".to_owned();
                draft.tax_office = "Çankaya".to_owned();
                draft.tax_number = "1234567890".to_owned();
                draft.contact_person = "Ayşe Öztürk".to_owned();
                draft.notes = "Ödeme günü: 15".to_owned();
            }))
            .expect("kayıt eklenebilmeli");

        let customer = database
            .customer_get(id)
            .expect("okuma başarılı olmalı")
            .expect("kayıt bulunmalı");

        assert_eq!(customer.code, "CAR-0001");
        assert_eq!(customer.name, "Öztürk İnşaat");
        assert_eq!(customer.kind, CustomerType::Both);
        assert_eq!(
            customer.phone.as_deref(),
            Some("0212 555 12 34"),
            "telefon baştaki sıfır korunarak metin olarak saklanmalı"
        );
        assert_eq!(customer.email.as_deref(), Some("info@ozturk.com"));
        assert_eq!(customer.address.as_deref(), Some("Ankara"));
        assert_eq!(customer.tax_office.as_deref(), Some("Çankaya"));
        assert_eq!(customer.tax_number.as_deref(), Some("1234567890"));
        assert_eq!(customer.national_id, None);
        assert_eq!(customer.contact_person.as_deref(), Some("Ayşe Öztürk"));
        assert_eq!(customer.notes.as_deref(), Some("Ödeme günü: 15"));
        assert!(customer.active);
        assert!(!customer.created_at.is_empty());
        assert_eq!(customer.created_at, customer.updated_at);
        assert_eq!(database.customer_count().unwrap(), 1);
    }

    #[test]
    fn olmayan_cari_hesap_none_doner() {
        let database = database();
        assert_eq!(database.customer_get(42).unwrap(), None);
        assert_eq!(database.customer_count().unwrap(), 0);
    }

    #[test]
    fn ayni_kod_ikinci_kez_eklenemez_ve_anlasilir_hata_doner() {
        let database = database();
        database
            .customer_insert(&valid(|draft| draft.code = "CAR-0001".to_owned()))
            .expect("ilk kayıt");

        let error = database
            .customer_insert(&valid(|draft| {
                draft.code = "CAR-0001".to_owned();
                draft.name = "Başka firma".to_owned();
            }))
            .expect_err("aynı kod reddedilmeli");

        assert!(
            matches!(&error, AppError::CustomerCodeTaken(code) if code == "CAR-0001"),
            "beklenen hata cari kodu çakışması olmalı, gelen: {error:?}"
        );
        assert!(error.to_string().contains("CAR-0001"));
        assert_eq!(database.customer_count().unwrap(), 1);
    }

    #[test]
    fn kod_benzersizligi_buyuk_kucuk_harf_duyarsizdir() {
        let database = database();
        database
            .customer_insert(&valid(|draft| draft.code = "CAR-0001".to_owned()))
            .expect("ilk kayıt");

        let error = database
            .customer_insert(&valid(|draft| draft.code = "car-0001".to_owned()))
            .expect_err("farklı büyük/küçük harf reddedilmeli");

        assert!(matches!(error, AppError::CustomerCodeTaken(_)));
        assert!(database.customer_code_exists("CAR-0001", None).unwrap());
        assert!(database.customer_code_exists("car-0001", None).unwrap());
    }

    #[test]
    fn kod_varligi_kaydin_kendisini_disliyor() {
        let database = database();
        let id = database
            .customer_insert(&valid(|draft| draft.code = "CAR-0001".to_owned()))
            .unwrap();

        assert!(!database.customer_code_exists("CAR-0001", Some(id)).unwrap());
        assert!(!database.customer_code_exists("CAR-0002", Some(id)).unwrap());

        database
            .customer_insert(&valid(|draft| {
                draft.code = "CAR-0002".to_owned();
                draft.name = "İkinci".to_owned();
            }))
            .unwrap();
        assert!(database.customer_code_exists("CAR-0002", Some(id)).unwrap());
    }

    #[test]
    fn cari_hesap_guncellenir_ve_olusturulma_zamani_degismez() {
        let database = database();
        let id = database
            .customer_insert(&valid(|draft| draft.code = "CAR-0001".to_owned()))
            .unwrap();
        let before = database.customer_get(id).unwrap().unwrap();

        database
            .customer_update(
                id,
                &valid(|draft| {
                    draft.code = "CAR-0009".to_owned();
                    draft.name = "Yeni Unvan".to_owned();
                    draft.kind = CustomerType::Supplier;
                    draft.active = false;
                }),
            )
            .expect("güncelleme başarılı olmalı");

        let after = database.customer_get(id).unwrap().unwrap();
        assert_eq!(after.id, id);
        assert_eq!(after.code, "CAR-0009");
        assert_eq!(after.name, "Yeni Unvan");
        assert_eq!(after.kind, CustomerType::Supplier);
        assert!(!after.active);
        assert_eq!(
            after.created_at, before.created_at,
            "oluşturulma zamanı değişmemeli"
        );
        assert_eq!(database.customer_count().unwrap(), 1);
    }

    #[test]
    fn guncelleme_baska_kaydin_koduyla_cakisamaz() {
        let database = database();
        let first = database
            .customer_insert(&valid(|draft| draft.code = "CAR-0001".to_owned()))
            .unwrap();
        database
            .customer_insert(&valid(|draft| {
                draft.code = "CAR-0002".to_owned();
                draft.name = "İkinci".to_owned();
            }))
            .unwrap();

        let error = database
            .customer_update(first, &valid(|draft| draft.code = "CAR-0002".to_owned()))
            .expect_err("çakışan kodla güncelleme reddedilmeli");
        assert!(matches!(error, AppError::CustomerCodeTaken(_)));

        // Kendi koduyla güncelleme sorun olmamalı.
        database
            .customer_update(first, &valid(|draft| draft.code = "CAR-0001".to_owned()))
            .expect("kendi koduyla güncelleme başarılı olmalı");
    }

    #[test]
    fn olmayan_kayit_guncellenemez_veya_durumu_degistirilemez() {
        let database = database();

        let update_error = database
            .customer_update(99, &valid(|_| {}))
            .expect_err("olmayan kayıt güncellenememeli");
        assert!(matches!(update_error, AppError::CustomerNotFound(99)));

        let active_error = database
            .customer_set_active(99, false)
            .expect_err("olmayan kaydın durumu değişmemeli");
        assert!(matches!(active_error, AppError::CustomerNotFound(99)));
    }

    #[test]
    fn pasife_alma_kaydi_silmez() {
        let database = database();
        let id = database
            .customer_insert(&valid(|draft| draft.code = "CAR-0001".to_owned()))
            .unwrap();

        database.customer_set_active(id, false).unwrap();
        let customer = database.customer_get(id).unwrap().unwrap();
        assert!(!customer.active);
        assert_eq!(customer.status_label(), "Pasif");
        assert_eq!(database.customer_count().unwrap(), 1, "kayıt silinmemeli");

        database.customer_set_active(id, true).unwrap();
        assert!(database.customer_get(id).unwrap().unwrap().active);
    }

    #[test]
    fn listeleme_tur_durum_ve_arama_olcutlerine_uyar() {
        let database = database();
        database
            .customer_insert(&valid(|draft| {
                draft.code = "CAR-0001".to_owned();
                draft.name = "Öztürk İnşaat".to_owned();
                draft.kind = CustomerType::Customer;
                draft.phone = "0212 555 12 34".to_owned();
            }))
            .unwrap();
        database
            .customer_insert(&valid(|draft| {
                draft.code = "CAR-0002".to_owned();
                draft.name = "Yılmaz Gıda".to_owned();
                draft.kind = CustomerType::Supplier;
            }))
            .unwrap();
        database
            .customer_insert(&valid(|draft| {
                draft.code = "CAR-0003".to_owned();
                draft.name = "Pasif Firma".to_owned();
                draft.active = false;
            }))
            .unwrap();

        // Ölçüt yok: tümü, kod sırasına göre.
        let all = database.customer_list(&CustomerFilter::default()).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].code, "CAR-0001");
        assert_eq!(all[2].code, "CAR-0003");

        // Türe göre.
        let suppliers = database
            .customer_list(&CustomerFilter {
                kind: Some(CustomerType::Supplier),
                ..CustomerFilter::default()
            })
            .unwrap();
        assert_eq!(suppliers.len(), 1);
        assert_eq!(suppliers[0].code, "CAR-0002");

        // Duruma göre.
        let inactive = database
            .customer_list(&CustomerFilter {
                active: Some(false),
                ..CustomerFilter::default()
            })
            .unwrap();
        assert_eq!(inactive.len(), 1);
        assert_eq!(inactive[0].code, "CAR-0003");

        let active = database
            .customer_list(&CustomerFilter {
                active: Some(true),
                ..CustomerFilter::default()
            })
            .unwrap();
        assert_eq!(active.len(), 2);

        // Ada göre arama (Türkçe karakter ve büyük/küçük harf duyarsız).
        for needle in ["öztürk", "OZTURK", "İnşaat", "insaat"] {
            let found = database
                .customer_list(&CustomerFilter {
                    search: needle.to_owned(),
                    ..CustomerFilter::default()
                })
                .unwrap();
            assert_eq!(found.len(), 1, "'{needle}' araması bir kayıt bulmalı");
            assert_eq!(found[0].code, "CAR-0001");
        }

        // Koda veya telefona göre arama.
        for needle in ["CAR-0002", "car-0002", "555 12 34"] {
            let found = database
                .customer_list(&CustomerFilter {
                    search: needle.to_owned(),
                    ..CustomerFilter::default()
                })
                .unwrap();
            assert!(!found.is_empty(), "'{needle}' araması sonuç bulmalı");
        }

        // Ölçütler birlikte uygulanır.
        let combined = database
            .customer_list(&CustomerFilter {
                search: "firma".to_owned(),
                kind: Some(CustomerType::Supplier),
                active: None,
            })
            .unwrap();
        assert!(combined.is_empty(), "tür ve arama birlikte uygulanmalı");

        // Sonuç yok.
        let empty = database
            .customer_list(&CustomerFilter {
                search: "bulunmayan-kayit".to_owned(),
                ..CustomerFilter::default()
            })
            .unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn kod_onerisi_veritabanindaki_kodlari_dikkate_alir() {
        let database = database();
        assert_eq!(database.customer_next_code().unwrap(), "CAR-0001");

        database
            .customer_insert(&valid(|draft| draft.code = "CAR-0005".to_owned()))
            .unwrap();
        assert_eq!(database.customer_next_code().unwrap(), "CAR-0006");
    }

    #[test]
    fn bos_alanlar_veritabaninda_null_olarak_saklanir() {
        let database = database();
        let id = database
            .customer_insert(&valid(|draft| {
                draft.phone = "   ".to_owned();
                draft.email = String::new();
            }))
            .unwrap();

        let is_null: bool = database
            .connection
            .query_row(
                "SELECT phone IS NULL AND email IS NULL FROM customers WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(is_null, "boş alanlar NULL olarak saklanmalı");
    }
}
