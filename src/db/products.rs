// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ürün ve stok veri erişimi.
//!
//! SQL ifadeleri yalnızca bu modülde bulunur; arayüz katmanı [`Database`]
//! üzerindeki metotları çağırır ve hiçbir zaman SQL yazmaz.
//!
//! ## Güvenlik ve bütünlük
//!
//! * Tüm kullanıcı girdileri **parametreli** sorgularla (`?1`, `?2`, ...)
//!   iletilir.
//! * Yazma metotları yalnızca [`ValidatedProduct`] / [`ValidatedMovement`]
//!   kabul eder; doğrulanmamış veri bu katmana gelemez.
//! * `code` ve (doluysa) `barcode` benzersizliği veritabanı düzeyinde korunur.
//! * `current_stock`, stok hareketiyle **aynı işlem (transaction) içinde**
//!   güncellenir; böylece `current_stock` her zaman hareketlerin toplamına
//!   eşittir.
//! * Stok hareketi silinmez/düzenlenmez; düzeltme ters yönlü bir `duzeltme`
//!   hareketiyle yapılır.

use rusqlite::types::Type;
use rusqlite::{OptionalExtension, Row, params};

use crate::error::{AppError, Result};
use crate::money::Money;
use crate::product::{
    MovementType, Product, ProductFilter, StockMovement, Unit, ValidatedMovement, ValidatedProduct,
    suggest_next_code,
};
use crate::quantity::Quantity;

use super::Database;

/// Ürün sorgularında kullanılan kolon listesi (sırası [`row_to_product`] ile aynıdır).
const PRODUCT_COLUMNS: &str = "id, code, barcode, name, unit, category, purchase_price, \
                               sale_price, vat_rate, min_stock, current_stock, notes, active, \
                               created_at, updated_at";

/// Stok hareketi sorgularında kullanılan kolon listesi ([`row_to_movement`] ile aynı).
const MOVEMENT_COLUMNS: &str = "m.id, m.product_id, p.code, p.name, p.unit, m.movement_type, \
                                m.quantity_delta, m.movement_date, m.description, m.created_at";

/// Ürün satırında birim kolonunun indeksi.
const UNIT_INDEX: usize = 4;

/// Stok hareketi satırında tür kolonunun indeksi.
const MOVEMENT_TYPE_INDEX: usize = 5;

impl Database {
    /// Ölçütlere uyan ürünleri ürün koduna göre sıralı döndürür.
    ///
    /// Durum ve düşük stok ölçütleri SQL'de (parametreli) uygulanır. Serbest
    /// metin araması, Türkçe karakterlerden etkilenmemesi için Rust tarafında
    /// yapılır.
    pub fn product_list(&self, filter: &ProductFilter) -> Result<Vec<Product>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {PRODUCT_COLUMNS}
             FROM products
             WHERE (?1 IS NULL OR active = ?1)
               AND (?2 = 0 OR (min_stock > 0 AND current_stock < min_stock))
             ORDER BY code COLLATE NOCASE"
        ))?;

        let active = filter.active.map(i64::from);
        let low_stock = i64::from(filter.low_stock_only);

        let rows = statement.query_map(params![active, low_stock], row_to_product)?;

        let mut products = Vec::new();
        for row in rows {
            products.push(row?);
        }

        let needle = crate::customer::normalize_for_search(filter.search.trim());
        if !needle.is_empty() {
            products.retain(|product| product.matches_search(&needle));
        }

        Ok(products)
    }

    /// Kimliğine göre tek bir ürün okur.
    pub fn product_get(&self, id: i64) -> Result<Option<Product>> {
        let product = self
            .connection
            .query_row(
                &format!("SELECT {PRODUCT_COLUMNS} FROM products WHERE id = ?1"),
                [id],
                row_to_product,
            )
            .optional()?;
        Ok(product)
    }

    /// Toplam ürün sayısı (pasif kayıtlar dâhil).
    pub fn product_count(&self) -> Result<usize> {
        let count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM products", [], |row| row.get(0))?;
        Ok(count.max(0) as usize)
    }

    /// Aktif ürün sayısı.
    pub fn active_product_count(&self) -> Result<usize> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM products WHERE active = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Düşük stoklu ürün sayısı (`min_stock > 0` ve `current_stock < min_stock`).
    pub fn low_stock_count(&self) -> Result<usize> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM products WHERE min_stock > 0 AND current_stock < min_stock",
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Ürün kodu kullanımda mı?
    ///
    /// `exclude_id` verilirse bu kayıt dışındaki kayıtlar denetlenir. Karşılaştırma
    /// büyük/küçük harf duyarsızdır; veritabanındaki `UNIQUE` kısıtı ile aynı
    /// kuralı uygular.
    pub fn product_code_exists(&self, code: &str, exclude_id: Option<i64>) -> Result<bool> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM products
                 WHERE code = ?1 COLLATE NOCASE AND (?2 IS NULL OR id <> ?2)
             )",
            params![code, exclude_id],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    /// Barkod başka bir üründe kullanımda mı?
    pub fn product_barcode_exists(&self, barcode: &str, exclude_id: Option<i64>) -> Result<bool> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM products
                 WHERE barcode = ?1 COLLATE NOCASE AND (?2 IS NULL OR id <> ?2)
             )",
            params![barcode, exclude_id],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    /// Kullanılmamış bir ürün kodu önerir (`URN-0001`, `URN-0002`, ...).
    pub fn product_next_code(&self) -> Result<String> {
        let mut statement = self.connection.prepare("SELECT code FROM products")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;

        let mut codes = Vec::new();
        for row in rows {
            codes.push(row?);
        }

        Ok(suggest_next_code(&codes))
    }

    /// Yeni ürün ekler ve oluşturulan kaydın kimliğini döndürür.
    ///
    /// Başlangıç stoğu girilmişse aynı işlem içinde `ilk_stok` hareketi
    /// oluşturulur ve `current_stock` bu değere ayarlanır.
    pub fn product_insert(&self, product: &ValidatedProduct) -> Result<i64> {
        let transaction = self.connection.unchecked_transaction()?;

        transaction
            .execute(
                "INSERT INTO products
                     (code, barcode, name, unit, category, purchase_price, sale_price,
                      vat_rate, min_stock, current_stock, notes, active)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    product.code,
                    product.barcode.as_deref(),
                    product.name,
                    product.unit.as_db(),
                    product.category.as_deref(),
                    product.purchase_price.kurus(),
                    product.sale_price.kurus(),
                    product.vat_rate,
                    product.min_stock.milli(),
                    product.starting_stock.milli(),
                    product.notes.as_deref(),
                    i64::from(product.active),
                ],
            )
            .map_err(|error| map_product_write_error(error, product))?;

        let id = transaction.last_insert_rowid();

        if !product.starting_stock.is_zero() {
            transaction.execute(
                "INSERT INTO product_stock_movements
                     (product_id, movement_type, quantity_delta, movement_date, description)
                 VALUES (?1, 'ilk_stok', ?2, date('now'), ?3)",
                params![
                    id,
                    product.starting_stock.milli(),
                    "Ürün kartı oluşturulurken girilen başlangıç stoğu.",
                ],
            )?;
        }

        transaction.commit()?;
        Ok(id)
    }

    /// Var olan ürünü günceller.
    ///
    /// `current_stock` bu metotla **değişmez**; stok yalnızca hareketlerle
    /// güncellenir. `updated_at` veritabanı saatine göre yenilenir; `created_at`
    /// değişmez. Kayıt yoksa [`AppError::ProductNotFound`] döner.
    pub fn product_update(&self, id: i64, product: &ValidatedProduct) -> Result<()> {
        let current = self.product_get(id)?.ok_or(AppError::ProductNotFound(id))?;
        if !self.session_is_manager() && current.active != product.active {
            tracing::warn!(user_id = id, "yetki reddedildi: ürün durumu değiştirme");
            return Err(AppError::PermissionDenied {
                operation: "ürün durumunu değiştirme".to_owned(),
            });
        }
        let transaction = self.connection.unchecked_transaction()?;

        let affected = transaction
            .execute(
                "UPDATE products
                    SET code = ?2,
                        barcode = ?3,
                        name = ?4,
                        unit = ?5,
                        category = ?6,
                        purchase_price = ?7,
                        sale_price = ?8,
                        vat_rate = ?9,
                        min_stock = ?10,
                        notes = ?11,
                        active = ?12,
                        updated_at = datetime('now')
                  WHERE id = ?1",
                params![
                    id,
                    product.code,
                    product.barcode.as_deref(),
                    product.name,
                    product.unit.as_db(),
                    product.category.as_deref(),
                    product.purchase_price.kurus(),
                    product.sale_price.kurus(),
                    product.vat_rate,
                    product.min_stock.milli(),
                    product.notes.as_deref(),
                    i64::from(product.active),
                ],
            )
            .map_err(|error| map_product_write_error(error, product))?;

        if affected == 0 {
            return Err(AppError::ProductNotFound(id));
        }

        transaction.commit()?;
        Ok(())
    }

    /// Ürünü aktif veya pasif yapar.
    ///
    /// Kayıt silinmez; ileride bağlanacak fatura satırlarının geçmişi korunur.
    pub fn product_set_active(&self, id: i64, active: bool) -> Result<()> {
        self.require_admin("ürün pasife alma/aktifleştirme")?;
        let affected = self.connection.execute(
            "UPDATE products
                SET active = ?2, updated_at = datetime('now')
              WHERE id = ?1",
            params![id, i64::from(active)],
        )?;

        if affected == 0 {
            return Err(AppError::ProductNotFound(id));
        }

        Ok(())
    }

    /// Yeni stok hareketi ekler ve ürünün güncel stoğunu aynı işlemde günceller.
    pub fn movement_insert(&self, product_id: i64, movement: &ValidatedMovement) -> Result<i64> {
        let transaction = self.connection.unchecked_transaction()?;

        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1)",
            [product_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::ProductNotFound(product_id));
        }

        transaction.execute(
            "INSERT INTO product_stock_movements
                 (product_id, movement_type, quantity_delta, movement_date, description)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                product_id,
                movement.movement_type.as_db(),
                movement.quantity_delta.milli(),
                movement.movement_date,
                movement.description.as_deref(),
            ],
        )?;

        let id = transaction.last_insert_rowid();

        transaction.execute(
            "UPDATE products
                SET current_stock = current_stock + ?2, updated_at = datetime('now')
              WHERE id = ?1",
            params![product_id, movement.quantity_delta.milli()],
        )?;

        transaction.commit()?;
        Ok(id)
    }

    /// Bir ürünün stok hareketlerini tarihe göre azalan sırada döndürür.
    pub fn movement_list_for_product(&self, product_id: i64) -> Result<Vec<StockMovement>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {MOVEMENT_COLUMNS}
             FROM product_stock_movements m
             JOIN products p ON p.id = m.product_id
             WHERE m.product_id = ?1
             ORDER BY m.movement_date DESC, m.id DESC"
        ))?;

        let rows = statement.query_map([product_id], row_to_movement)?;

        let mut movements = Vec::new();
        for row in rows {
            movements.push(row?);
        }
        Ok(movements)
    }

    /// Son stok hareketlerini (en yeni önce) döndürür.
    pub fn recent_movements(&self, limit: usize) -> Result<Vec<StockMovement>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {MOVEMENT_COLUMNS}
             FROM product_stock_movements m
             JOIN products p ON p.id = m.product_id
             ORDER BY m.movement_date DESC, m.id DESC
             LIMIT ?1"
        ))?;

        let rows = statement.query_map([limit as i64], row_to_movement)?;

        let mut movements = Vec::new();
        for row in rows {
            movements.push(row?);
        }
        Ok(movements)
    }
}

/// Bir veritabanı satırını [`Product`] tipine dönüştürür.
fn row_to_product(row: &Row<'_>) -> rusqlite::Result<Product> {
    let unit_value: String = row.get(UNIT_INDEX)?;
    let unit = Unit::from_db(&unit_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            UNIT_INDEX,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen ölçü birimi: '{unit_value}'"
            ))),
        )
    })?;

    Ok(Product {
        id: row.get(0)?,
        code: row.get(1)?,
        barcode: row.get(2)?,
        name: row.get(3)?,
        unit,
        category: row.get(5)?,
        purchase_price: Money::from_kurus(row.get(6)?),
        sale_price: Money::from_kurus(row.get(7)?),
        vat_rate: row.get(8)?,
        min_stock: Quantity::from_milli(row.get(9)?),
        current_stock: Quantity::from_milli(row.get(10)?),
        notes: row.get(11)?,
        active: row.get::<_, i64>(12)? != 0,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

/// Bir veritabanı satırını [`StockMovement`] tipine dönüştürür.
fn row_to_movement(row: &Row<'_>) -> rusqlite::Result<StockMovement> {
    let type_value: String = row.get(MOVEMENT_TYPE_INDEX)?;
    let movement_type = MovementType::from_db(&type_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            MOVEMENT_TYPE_INDEX,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen hareket türü: '{type_value}'"
            ))),
        )
    })?;

    let unit_value: String = row.get(4)?;
    let product_unit = Unit::from_db(&unit_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen ölçü birimi: '{unit_value}'"
            ))),
        )
    })?;

    Ok(StockMovement {
        id: row.get(0)?,
        product_id: row.get(1)?,
        product_code: row.get(2)?,
        product_name: row.get(3)?,
        product_unit,
        movement_type,
        quantity_delta: Quantity::from_milli(row.get(6)?),
        movement_date: row.get(7)?,
        description: row.get(8)?,
        created_at: row.get(9)?,
    })
}

/// Yazma hatalarını kullanıcıya anlamlı gelen hatalara dönüştürür.
///
/// `code` ve `barcode` benzersizlik ihlalleri SQLite'ın hata mesajından ayırt
/// edilir; diğer kısıt ihlalleri genel veritabanı hatası olarak döner.
fn map_product_write_error(error: rusqlite::Error, product: &ValidatedProduct) -> AppError {
    if let rusqlite::Error::SqliteFailure(inner, message) = &error
        && inner.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    {
        let text = message.as_deref().unwrap_or_default();
        if text.contains("barcode") {
            return AppError::ProductBarcodeTaken(product.barcode.clone().unwrap_or_default());
        }
        return AppError::ProductCodeTaken(product.code.clone());
    }

    AppError::Database(crate::error::sqlite_error_to_turkish(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::{MovementDraft, ProductDraft};

    fn valid(overrides: impl FnOnce(&mut ProductDraft)) -> ValidatedProduct {
        let mut draft = ProductDraft {
            code: "URN-0001".to_owned(),
            name: "Vida 4x40".to_owned(),
            ..ProductDraft::default()
        };
        overrides(&mut draft);
        draft.validate().expect("test verisi geçerli olmalı")
    }

    fn database() -> Database {
        Database::open_in_memory().expect("bellek içi veritabanı açılmalı")
    }

    fn movement(movement_type: MovementType, quantity: i64, text: &str) -> ValidatedMovement {
        let mut draft = MovementDraft {
            movement_type,
            quantity: Quantity::from_milli(quantity).format_turkish(),
            date: "2026-10-18".to_owned(),
            description: text.to_owned(),
            ..MovementDraft::default()
        };
        if movement_type == MovementType::Adjust {
            draft.adjustment_negative = true;
        }
        draft.validate().expect("hareket geçerli olmalı")
    }

    #[test]
    fn urun_eklenir_ve_okunur() {
        let database = database();

        let id = database
            .product_insert(&valid(|draft| {
                draft.code = "URN-0001".to_owned();
                draft.barcode = "8691234567890".to_owned();
                draft.name = "Çelik Vida".to_owned();
                draft.unit = Unit::Kilogram;
                draft.category = "Bağlantı".to_owned();
                draft.purchase_price = "10,50".to_owned();
                draft.sale_price = "15".to_owned();
                draft.vat_rate = "20".to_owned();
                draft.min_stock = "5".to_owned();
                draft.notes = "Raf A1".to_owned();
                draft.starting_stock = "12,5".to_owned();
            }))
            .expect("ürün eklenebilmeli");

        let product = database.product_get(id).unwrap().unwrap();
        assert_eq!(product.code, "URN-0001");
        assert_eq!(product.barcode.as_deref(), Some("8691234567890"));
        assert_eq!(product.name, "Çelik Vida");
        assert_eq!(product.unit, Unit::Kilogram);
        assert_eq!(product.purchase_price.kurus(), 1050);
        assert_eq!(product.sale_price.kurus(), 1500);
        assert_eq!(product.min_stock.milli(), 5000);
        assert_eq!(
            product.current_stock.milli(),
            12_500,
            "başlangıç stoğu current_stock'a yazılmalı"
        );
        assert!(product.active);
        assert_eq!(database.product_count().unwrap(), 1);

        // Başlangıç stoğu bir `ilk_stok` hareketi üretmeli.
        let movements = database.movement_list_for_product(id).unwrap();
        assert_eq!(movements.len(), 1);
        assert_eq!(movements[0].movement_type, MovementType::Initial);
        assert_eq!(movements[0].quantity_delta.milli(), 12_500);
    }

    #[test]
    fn baslangic_stogu_olmayan_urun_hareket_uretmez() {
        let database = database();
        let id = database.product_insert(&valid(|_| {})).unwrap();
        assert!(database.movement_list_for_product(id).unwrap().is_empty());
        assert_eq!(
            database
                .product_get(id)
                .unwrap()
                .unwrap()
                .current_stock
                .milli(),
            0
        );
    }

    #[test]
    fn olmayan_urun_none_doner() {
        let database = database();
        assert_eq!(database.product_get(42).unwrap(), None);
        assert_eq!(database.product_count().unwrap(), 0);
    }

    #[test]
    fn ayni_kod_ikinci_kez_eklenemez() {
        let database = database();
        database
            .product_insert(&valid(|draft| draft.code = "URN-0001".to_owned()))
            .expect("ilk kayıt");

        let error = database
            .product_insert(&valid(|draft| draft.code = "urn-0001".to_owned()))
            .expect_err("farklı büyük/küçük harf reddedilmeli");
        assert!(
            matches!(&error, AppError::ProductCodeTaken(code) if code == "urn-0001"),
            "beklenen hata ürün kodu çakışması olmalı: {error:?}"
        );
        assert!(database.product_code_exists("URN-0001", None).unwrap());
        assert!(database.product_code_exists("urn-0001", None).unwrap());
    }

    #[test]
    fn ayni_barkod_ikinci_kez_eklenemez_bos_barkod_serbesttir() {
        let database = database();
        database
            .product_insert(&valid(|draft| {
                draft.code = "URN-0001".to_owned();
                draft.barcode = "8691234567890".to_owned();
            }))
            .expect("ilk kayıt");

        let error = database
            .product_insert(&valid(|draft| {
                draft.code = "URN-0002".to_owned();
                draft.barcode = "8691234567890".to_owned();
            }))
            .expect_err("aynı barkod reddedilmeli");
        assert!(matches!(error, AppError::ProductBarcodeTaken(_)));

        // Barkodsuz birden çok ürün eklenebilmeli.
        database
            .product_insert(&valid(|draft| {
                draft.code = "URN-0003".to_owned();
                draft.barcode = String::new();
            }))
            .unwrap();
        database
            .product_insert(&valid(|draft| {
                draft.code = "URN-0004".to_owned();
                draft.barcode = String::new();
            }))
            .unwrap();
        assert_eq!(database.product_count().unwrap(), 3);
    }

    #[test]
    fn urun_guncellenir_ve_stok_degismez() {
        let database = database();
        let id = database
            .product_insert(&valid(|draft| draft.starting_stock = "10".to_owned()))
            .unwrap();
        let before = database.product_get(id).unwrap().unwrap();

        database
            .product_update(
                id,
                &valid(|draft| {
                    draft.code = "URN-0009".to_owned();
                    draft.name = "Yeni Ad".to_owned();
                    draft.unit = Unit::Litre;
                    draft.active = false;
                }),
            )
            .expect("güncelleme başarılı olmalı");

        let after = database.product_get(id).unwrap().unwrap();
        assert_eq!(after.code, "URN-0009");
        assert_eq!(after.name, "Yeni Ad");
        assert_eq!(after.unit, Unit::Litre);
        assert!(!after.active);
        assert_eq!(
            after.current_stock, before.current_stock,
            "stok yalnızca hareketlerle değişmeli"
        );
        assert_eq!(after.created_at, before.created_at);
    }

    #[test]
    fn olmayan_urun_guncellenemez_veya_durumu_degistirilemez() {
        let database = database();
        assert!(matches!(
            database.product_update(99, &valid(|_| {})),
            Err(AppError::ProductNotFound(99))
        ));
        assert!(matches!(
            database.product_set_active(99, false),
            Err(AppError::ProductNotFound(99))
        ));
        assert!(matches!(
            database.movement_insert(99, &movement(MovementType::In, 1000, "")),
            Err(AppError::ProductNotFound(99))
        ));
    }

    #[test]
    fn pasife_alma_kaydi_silmez() {
        let database = database();
        let id = database.product_insert(&valid(|_| {})).unwrap();
        database.product_set_active(id, false).unwrap();
        assert!(!database.product_get(id).unwrap().unwrap().active);
        assert_eq!(database.product_count().unwrap(), 1, "kayıt silinmemeli");
        assert_eq!(database.active_product_count().unwrap(), 0);
    }

    #[test]
    fn ardisik_hareketler_sonrasi_stok_hareket_toplamina_esittir() {
        let database = database();
        let id = database.product_insert(&valid(|_| {})).unwrap();

        database
            .movement_insert(id, &movement(MovementType::In, 10_000, "Alış"))
            .unwrap();
        database
            .movement_insert(id, &movement(MovementType::Out, 4_000, "Satış"))
            .unwrap();
        database
            .movement_insert(id, &movement(MovementType::Adjust, 2_500, "Sayım"))
            .unwrap();

        let product = database.product_get(id).unwrap().unwrap();
        let sum: i64 = database
            .connection
            .query_row(
                "SELECT COALESCE(SUM(quantity_delta), 0) FROM product_stock_movements WHERE product_id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(product.current_stock.milli(), sum);
        assert_eq!(product.current_stock.milli(), 10_000 - 4_000 - 2_500);
    }

    #[test]
    fn dusuk_stok_sayimi_ve_filtresi_calisir() {
        let database = database();
        database
            .product_insert(&valid(|draft| {
                draft.code = "URN-0001".to_owned();
                draft.min_stock = "10".to_owned();
                draft.starting_stock = "4".to_owned();
            }))
            .unwrap();
        database
            .product_insert(&valid(|draft| {
                draft.code = "URN-0002".to_owned();
                draft.min_stock = "10".to_owned();
                draft.starting_stock = "20".to_owned();
            }))
            .unwrap();
        // Minimumu tanımlanmamış ürün düşük stok sayılmaz.
        database
            .product_insert(&valid(|draft| {
                draft.code = "URN-0003".to_owned();
            }))
            .unwrap();

        assert_eq!(database.low_stock_count().unwrap(), 1);

        let low = database
            .product_list(&ProductFilter {
                low_stock_only: true,
                ..ProductFilter::default()
            })
            .unwrap();
        assert_eq!(low.len(), 1);
        assert_eq!(low[0].code, "URN-0001");
    }

    #[test]
    fn listeleme_durum_ve_arama_olcutlerine_uyar() {
        let database = database();
        database
            .product_insert(&valid(|draft| {
                draft.code = "URN-0001".to_owned();
                draft.name = "Çelik Vida".to_owned();
                draft.barcode = "8691234567890".to_owned();
                draft.category = "Bağlantı".to_owned();
            }))
            .unwrap();
        database
            .product_insert(&valid(|draft| {
                draft.code = "URN-0002".to_owned();
                draft.name = "Bakır Kablo".to_owned();
                draft.active = false;
            }))
            .unwrap();

        let all = database.product_list(&ProductFilter::default()).unwrap();
        assert_eq!(all.len(), 2);

        let active = database
            .product_list(&ProductFilter {
                active: Some(true),
                ..ProductFilter::default()
            })
            .unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].code, "URN-0001");

        for needle in ["çelik", "CELIK", "celik", "8691234567890", "baglanti"] {
            let found = database
                .product_list(&ProductFilter {
                    search: needle.to_owned(),
                    ..ProductFilter::default()
                })
                .unwrap();
            assert_eq!(found.len(), 1, "'{needle}' tek kayıt bulmalı");
            assert_eq!(found[0].code, "URN-0001");
        }

        let empty = database
            .product_list(&ProductFilter {
                search: "bulunmayan".to_owned(),
                ..ProductFilter::default()
            })
            .unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn son_hareketler_en_yeniden_eskiye_siralanir() {
        let database = database();
        let id = database.product_insert(&valid(|_| {})).unwrap();

        let mut first = movement(MovementType::In, 1000, "bir");
        first.movement_date = "2026-10-01".to_owned();
        database.movement_insert(id, &first).unwrap();

        let mut second = movement(MovementType::In, 2000, "iki");
        second.movement_date = "2026-10-10".to_owned();
        database.movement_insert(id, &second).unwrap();

        let recent = database.recent_movements(5).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(
            recent[0].movement_date, "2026-10-10",
            "en yeni önce gelmeli"
        );
        assert_eq!(recent[0].product_code, "URN-0001");
    }

    #[test]
    fn kod_onerisi_veritabanindaki_kodlari_dikkate_alir() {
        let database = database();
        assert_eq!(database.product_next_code().unwrap(), "URN-0001");

        database
            .product_insert(&valid(|draft| draft.code = "URN-0005".to_owned()))
            .unwrap();
        assert_eq!(database.product_next_code().unwrap(), "URN-0006");
    }
}
