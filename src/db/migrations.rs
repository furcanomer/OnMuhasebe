// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Veritabanı şemasının sürümlenmesi (migration altyapısı).
//!
//! ## Nasıl çalışır?
//!
//! * [`MIGRATIONS`] listesi, uygulanacak tüm şema değişikliklerini sırayla tutar.
//! * Her migration'ın `version` numarası tek ve artan olmalıdır.
//! * Uygulanan migration'lar `schema_migrations` tablosuna yazılır.
//! * [`apply_pending`] yalnızca henüz uygulanmamış migration'ları çalıştırır ve
//!   her birini **kendi işlemi (transaction) içinde** uygular; böylece yarıda
//!   kalan bir migration veritabanını tutarsız bırakmaz.
//!
//! ## Yeni migration ekleme
//!
//! 1. [`MIGRATIONS`] listesinin **sonuna** yeni bir `Migration` kaydı ekleyin.
//! 2. `version` değerini bir öncekinin bir fazlası yapın.
//! 3. SQL'i `IF NOT EXISTS` kullanmadan yazın; migration yalnızca bir kez çalışır.
//!
//! **Uygulanmış bir migration'ın SQL'i asla değiştirilmemelidir**; aksi hâlde
//! var olan veritabanları ile yeni kurulanlar farklı şemalara sahip olur.
//!
//! Şu an uygulanan sürümler:
//!
//! | Sürüm | İçerik                                          |
//! |-------|-------------------------------------------------|
//! | 1     | Uygulama meta verisi (`app_metadata`)           |
//! | 2     | Cari hesaplar (`customers`)                     |
//! | 3     | Ürünler ve stok hareketleri                     |
//! | 4     | Alış/satış faturaları (`invoices`, `invoice_lines`) |
//! | 5     | Kasa/banka ve tahsilat/ödeme (`cash_accounts`, `payments`, `payment_allocations`) |
//! | 6     | Gelir/gider kategorileri ve hareketleri (`expense_categories`, `income_expense_entries`) |
//! | 7     | Kullanıcılar ve uygulama ayarları (`users`, `app_settings`) |

use rusqlite::{Connection, params};

use crate::error::{AppError, Result};

/// Tek bir şema değişikliğini tanımlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    /// Sıra numarası; tek ve artan olmalıdır.
    pub version: i32,
    /// İnsan tarafından okunabilir ad.
    pub name: &'static str,
    /// Çalıştırılacak SQL.
    pub sql: &'static str,
}

/// Uygulanmış bir migration kaydı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    /// Migration sıra numarası.
    pub version: i32,
    /// Migration adı.
    pub name: String,
    /// Uygulanma zamanı (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub applied_at: String,
}

/// Bilinen tüm migration'lar. Yeni şema değişiklikleri listenin **sonuna** eklenir.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "uygulama_meta_verisi",
        sql: MIGRATION_0001_APP_METADATA,
    },
    Migration {
        version: 2,
        name: "cari_hesaplar",
        sql: MIGRATION_0002_CUSTOMERS,
    },
    Migration {
        version: 3,
        name: "urunler_ve_stok",
        sql: MIGRATION_0003_PRODUCTS,
    },
    Migration {
        version: 4,
        name: "faturalar",
        sql: MIGRATION_0004_INVOICES,
    },
    Migration {
        version: 5,
        name: "kasa_banka_ve_odemeler",
        sql: MIGRATION_0005_CASH_AND_PAYMENTS,
    },
    Migration {
        version: 6,
        name: "gelir_gider_kategorileri_ve_hareketleri",
        sql: MIGRATION_0006_INCOME_EXPENSE,
    },
    Migration {
        version: 7,
        name: "kullanicilar_ve_uygulama_ayarlari",
        sql: MIGRATION_0007_USERS_AND_SETTINGS,
    },
];

/// 1 numaralı migration: uygulama meta verisi.
const MIGRATION_0001_APP_METADATA: &str = r#"
CREATE TABLE IF NOT EXISTS app_metadata (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

/// 2 numaralı migration: cari hesaplar.
///
/// Tasarım kararları:
///
/// * `code` **benzersizdir** ve büyük/küçük harf duyarsız karşılaştırılır
///   (`COLLATE NOCASE`), böylece `CAR-0001` ile `car-0001` çakışır.
/// * `AUTOINCREMENT` kullanılır: silinen bir kaydın kimliği bir daha
///   kullanılmaz. Sonraki aşamalarda cari hareketler bu `id` değerine bağlı
///   olacağı için kimliklerin geri dönüşmemesi veri bütünlüğü açısından
///   önemlidir.
/// * Telefon ve kimlik numaraları **TEXT** olarak saklanır; böylece baştaki
///   sıfırlar korunur ve sayısal taşma/biçim sorunları oluşmaz.
/// * Metin alanlarında `CHECK` kısıtları ve `kind`/`active` alanlarında
///   `IN (...)` kısıtları vardır; geçersiz değer veritabanına yazılamaz.
/// * Kayıt **silinmez**: pasife alma için `active` kolonu kullanılır.
const MIGRATION_0002_CUSTOMERS: &str = r#"
CREATE TABLE customers (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    code           TEXT    NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(code)) > 0),
    name           TEXT    NOT NULL CHECK (length(trim(name)) > 0),
    kind           TEXT    NOT NULL CHECK (kind IN ('musteri', 'tedarikci', 'ikisi')),
    phone          TEXT,
    email          TEXT,
    address        TEXT,
    tax_office     TEXT,
    tax_number     TEXT,
    national_id    TEXT,
    contact_person TEXT,
    notes          TEXT,
    active         INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_customers_name ON customers (name COLLATE NOCASE);
CREATE INDEX idx_customers_active ON customers (active);
CREATE INDEX idx_customers_kind ON customers (kind);
"#;

/// 3 numaralı migration: ürünler ve stok hareketleri.
///
/// Tasarım kararları:
///
/// * `code` **benzersizdir** ve büyük/küçük harf duyarsız karşılaştırılır
///   (`COLLATE NOCASE`), tıpkı cari kodunda olduğu gibi.
/// * `barcode` isteğe bağlıdır; yalnızca dolu barkodlar için **kısmi benzersiz
///   indeks** tanımlıdır. Böylece birden çok ürün barkodsuz olabilir ama aynı
///   barkod iki kez kullanılamaz.
/// * Fiyatlar **kuruş**, stok miktarları **mili-birim** cinsinden tam sayıdır
///   (bkz. [`crate::money`] ve [`crate::quantity`]).
/// * `current_stock` yalnızca stok hareketleriyle ve hareketle **aynı işlem
///   (transaction) içinde** güncellenir; böylece hareket geçmişi ile güncel
///   stok her zaman tutarlıdır.
/// * Stok hareketi **silinmez/düzenlenmez**; düzeltme, ters yönlü bir
///   `duzeltme` hareketiyle yapılır. Bu nedenle `movement_type` kısıtı ileride
///   (alış/satış/iade) genişletilecek şekilde ayrı tutulmuştur.
/// * Kayıt **silinmez**: pasife alma için `active` kolonu kullanılır.
///
/// İleride fatura satırları `products(id)` ve `product_stock_movements`
/// tablolarına bağlanacaktır; bu nedenle `id` değerleri `AUTOINCREMENT` ile
/// geri dönüşmez.
const MIGRATION_0003_PRODUCTS: &str = r#"
CREATE TABLE products (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    code           TEXT    NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(code)) > 0),
    barcode        TEXT,
    name           TEXT    NOT NULL CHECK (length(trim(name)) > 0),
    unit           TEXT    NOT NULL CHECK (unit IN
        ('adet', 'kg', 'g', 'litre', 'ml', 'metre', 'm2', 'paket', 'kutu', 'koli', 'ton')),
    category       TEXT,
    purchase_price INTEGER NOT NULL DEFAULT 0 CHECK (purchase_price >= 0),
    sale_price     INTEGER NOT NULL DEFAULT 0 CHECK (sale_price >= 0),
    vat_rate       INTEGER NOT NULL DEFAULT 20 CHECK (vat_rate BETWEEN 0 AND 100),
    min_stock      INTEGER NOT NULL DEFAULT 0 CHECK (min_stock >= 0),
    current_stock  INTEGER NOT NULL DEFAULT 0,
    notes          TEXT,
    active         INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_products_name ON products (name COLLATE NOCASE);
CREATE INDEX idx_products_active ON products (active);
CREATE UNIQUE INDEX idx_products_barcode ON products (barcode)
    WHERE barcode IS NOT NULL AND barcode <> '';

CREATE TABLE product_stock_movements (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id     INTEGER NOT NULL REFERENCES products (id),
    movement_type  TEXT    NOT NULL CHECK (movement_type IN
        ('ilk_stok', 'giris', 'cikis', 'duzeltme')),
    quantity_delta INTEGER NOT NULL CHECK (quantity_delta <> 0),
    movement_date  TEXT    NOT NULL,
    description    TEXT,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_stock_movements_product ON product_stock_movements (product_id, movement_date);
CREATE INDEX idx_stock_movements_date ON product_stock_movements (movement_date);
"#;

/// 4 numaralı migration: alış/satış faturaları.
///
/// Tasarım kararları:
///
/// * `invoice_number` **benzersizdir** ve büyük/küçük harf duyarsız
///   karşılaştırılır (`COLLATE NOCASE`); `SF-2026-0001` ile `sf-2026-0001`
///   çakışır.
/// * Tutarlar **kuruş**, miktarlar **mili-birim** cinsinden tam sayıdır
///   (bkz. [`crate::money`] ve [`crate::quantity`]).
/// * Fatura satırları ürün bilgisini **kopyalar** (`product_code`,
///   `product_name`, `unit`). Böylece ürün kartı sonradan değişse bile geçmiş
///   belge değişmez.
/// * Fatura **silinmez/düzenlenmez**: iptal `status` ile işaretlenir ve stok
///   etkisi ters yönlü bir `duzeltme` hareketiyle geri alınır.
/// * `product_stock_movements` tablosu **yeniden oluşturulur**: fatura kaynaklı
///   hareket türleri (`alis_faturasi`, `satis_faturasi`) ve `invoice_id`
///   bağlantısı eklenir. SQLite `CHECK` kısıtını değiştiremediği için tablo
///   kopyalanır; **mevcut hareketler korunur**.
const MIGRATION_0004_INVOICES: &str = r#"
CREATE TABLE invoices (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_number TEXT    NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(invoice_number)) > 0),
    invoice_type   TEXT    NOT NULL CHECK (invoice_type IN ('alis', 'satis')),
    customer_id    INTEGER NOT NULL REFERENCES customers (id),
    invoice_date   TEXT    NOT NULL,
    due_date       TEXT,
    notes          TEXT,
    subtotal       INTEGER NOT NULL DEFAULT 0 CHECK (subtotal >= 0),
    discount_total INTEGER NOT NULL DEFAULT 0 CHECK (discount_total >= 0),
    net_total      INTEGER NOT NULL DEFAULT 0 CHECK (net_total >= 0),
    vat_total      INTEGER NOT NULL DEFAULT 0 CHECK (vat_total >= 0),
    grand_total    INTEGER NOT NULL DEFAULT 0 CHECK (grand_total >= 0),
    status         TEXT    NOT NULL DEFAULT 'kesin' CHECK (status IN ('kesin', 'iptal')),
    cancelled_at   TEXT,
    cancel_reason  TEXT,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_invoices_customer ON invoices (customer_id, invoice_date);
CREATE INDEX idx_invoices_type_date ON invoices (invoice_type, invoice_date);
CREATE INDEX idx_invoices_status ON invoices (status);

CREATE TABLE invoice_lines (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id    INTEGER NOT NULL REFERENCES invoices (id),
    product_id    INTEGER NOT NULL REFERENCES products (id),
    line_no       INTEGER NOT NULL CHECK (line_no > 0),
    product_code  TEXT    NOT NULL,
    product_name  TEXT    NOT NULL,
    unit          TEXT    NOT NULL CHECK (unit IN
        ('adet', 'kg', 'g', 'litre', 'ml', 'metre', 'm2', 'paket', 'kutu', 'koli', 'ton')),
    description   TEXT,
    quantity      INTEGER NOT NULL CHECK (quantity <> 0),
    unit_price    INTEGER NOT NULL CHECK (unit_price >= 0),
    discount_rate INTEGER NOT NULL DEFAULT 0 CHECK (discount_rate BETWEEN 0 AND 100),
    vat_rate      INTEGER NOT NULL CHECK (vat_rate BETWEEN 0 AND 100),
    line_gross    INTEGER NOT NULL,
    line_discount INTEGER NOT NULL,
    line_net      INTEGER NOT NULL,
    line_vat      INTEGER NOT NULL,
    line_total    INTEGER NOT NULL,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (invoice_id, line_no)
);

CREATE INDEX idx_invoice_lines_invoice ON invoice_lines (invoice_id);

CREATE TABLE product_stock_movements_new (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id     INTEGER NOT NULL REFERENCES products (id),
    movement_type  TEXT    NOT NULL CHECK (movement_type IN
        ('ilk_stok', 'giris', 'cikis', 'duzeltme', 'alis_faturasi', 'satis_faturasi')),
    quantity_delta INTEGER NOT NULL CHECK (quantity_delta <> 0),
    movement_date  TEXT    NOT NULL,
    description    TEXT,
    invoice_id     INTEGER REFERENCES invoices (id),
    created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO product_stock_movements_new
    (id, product_id, movement_type, quantity_delta, movement_date, description, created_at, invoice_id)
SELECT id, product_id, movement_type, quantity_delta, movement_date, description, created_at, NULL
FROM product_stock_movements;

DROP TABLE product_stock_movements;

ALTER TABLE product_stock_movements_new RENAME TO product_stock_movements;

CREATE INDEX idx_stock_movements_product ON product_stock_movements (product_id, movement_date);
CREATE INDEX idx_stock_movements_date ON product_stock_movements (movement_date);
"#;

/// 5 numaralı migration: kasa/banka hesapları ve tahsilat/ödeme kayıtları.
///
/// Tasarım kararları:
///
/// * `invoices` tablosuna **eklenen** `paid_amount` kolonu, faturalara
///   bağlanmış kesin tahsilat/ödeme dağıtımlarının toplamını tutar. Eklenen
///   kolon `NOT NULL DEFAULT 0` olduğu için **mevcut faturalar 0 değerini**
///   alır; geçmiş veri değişmez.
/// * Kasa/banka hesabı adı **benzersizdir** ve büyük/küçük harf duyarsızdır
///   (`COLLATE NOCASE`).
/// * `current_balance` **işaretli** olabilir (kasa eksiye düşebilir);
///   `opening_balance` ise negatif olamaz.
/// * Kayıt **silinmez**: kasa/banka hesabı `active` ile pasife alınır,
///   tahsilat/ödeme ise `status = 'iptal'` ile iptal edilir ve düzeltme yeni
///   bir kayıtla yapılır.
/// * `payment_allocations` bir ödemenin hangi faturaya ne kadar dağıtıldığını
///   tutar; `ON DELETE CASCADE` ve `UNIQUE (payment_id, invoice_id)` ile
///   bütünlük korunur.
const MIGRATION_0005_CASH_AND_PAYMENTS: &str = r#"
ALTER TABLE invoices ADD COLUMN paid_amount INTEGER NOT NULL DEFAULT 0;

CREATE TABLE cash_accounts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT    NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(name)) > 0),
    account_type    TEXT    NOT NULL CHECK (account_type IN ('kasa', 'banka')),
    bank_name       TEXT,
    iban            TEXT,
    opening_balance INTEGER NOT NULL DEFAULT 0 CHECK (opening_balance >= 0),
    current_balance INTEGER NOT NULL DEFAULT 0,
    notes           TEXT,
    active          INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_cash_accounts_active ON cash_accounts (active);

CREATE TABLE payments (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    payment_type    TEXT    NOT NULL CHECK (payment_type IN ('tahsilat', 'odeme')),
    customer_id     INTEGER NOT NULL REFERENCES customers (id),
    cash_account_id INTEGER NOT NULL REFERENCES cash_accounts (id),
    payment_date    TEXT    NOT NULL,
    amount          INTEGER NOT NULL CHECK (amount > 0),
    description     TEXT,
    status          TEXT    NOT NULL DEFAULT 'kesin' CHECK (status IN ('kesin', 'iptal')),
    cancelled_at    TEXT,
    cancel_reason   TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_payments_customer ON payments (customer_id, payment_date);
CREATE INDEX idx_payments_cash_account ON payments (cash_account_id, payment_date);
CREATE INDEX idx_payments_type_date ON payments (payment_type, payment_date);
CREATE INDEX idx_payments_status ON payments (status);

CREATE TABLE payment_allocations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    payment_id  INTEGER NOT NULL REFERENCES payments (id) ON DELETE CASCADE,
    invoice_id  INTEGER NOT NULL REFERENCES invoices (id),
    amount      INTEGER NOT NULL CHECK (amount > 0),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (payment_id, invoice_id)
);

CREATE INDEX idx_payment_allocations_payment ON payment_allocations (payment_id);
CREATE INDEX idx_payment_allocations_invoice ON payment_allocations (invoice_id);
CREATE INDEX idx_invoices_paid_amount ON invoices (paid_amount);
"#;

/// 6 numaralı migration: gelir/gider kategorileri ve hareketleri.
///
/// Tasarım kararları:
///
/// * Kategoriler **gelir** ve **gider** olmak üzere iki türlüdür; ad yalnızca
///   kendi türü içinde benzersizdir (`UNIQUE (name, category_type)`). `name`
///   `COLLATE NOCASE` olduğu için `Kira` ile `kira` çakışır.
/// * Kategori **silinmez**: `active` alanı ile pasife alınır. Pasif kategori
///   yeni hareketlerde seçilemez, mevcut kayıtlarda görünmeye devam eder.
/// * `income_expense_entries` yalnızca cari hesaba bağlı olmayan nakit
///   hareketlerini tutar; `customer_id`/`invoice_id` **yoktur**. Bu nedenle
///   gelir/gider kayıtları cari bakiyesini veya fatura ödenen tutarını
///   etkilemez.
/// * Tutar **kuruş** cinsindendir ve sıfırdan büyüktür. Kayıt **silinmez/
///   düzenlenmez**: düzeltme, iptal + yeni kayıt ile yapılır.
/// * Kayıt kesinleşirken hareket ve kasa/banka bakiyesi **tek bir işlem
///   (transaction) içinde** yazılır; iptalde bakiyenin etkisi aynı şekilde
///   geri alınır.
const MIGRATION_0006_INCOME_EXPENSE: &str = r#"
CREATE TABLE expense_categories (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT    NOT NULL COLLATE NOCASE CHECK (length(trim(name)) > 0),
    category_type TEXT    NOT NULL CHECK (category_type IN ('gelir', 'gider')),
    description   TEXT,
    active        INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (name, category_type)
);

CREATE INDEX idx_expense_categories_type_active ON expense_categories (category_type, active);
CREATE INDEX idx_expense_categories_name ON expense_categories (name COLLATE NOCASE);

CREATE TABLE income_expense_entries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_type      TEXT    NOT NULL CHECK (entry_type IN ('gelir', 'gider')),
    category_id     INTEGER NOT NULL REFERENCES expense_categories (id),
    cash_account_id INTEGER NOT NULL REFERENCES cash_accounts (id),
    entry_date      TEXT    NOT NULL,
    amount          INTEGER NOT NULL CHECK (amount > 0),
    description     TEXT,
    document_ref    TEXT,
    status          TEXT    NOT NULL DEFAULT 'kesin' CHECK (status IN ('kesin', 'iptal')),
    cancelled_at    TEXT,
    cancel_reason   TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_entries_type_date ON income_expense_entries (entry_type, entry_date);
CREATE INDEX idx_entries_category ON income_expense_entries (category_id, entry_date);
CREATE INDEX idx_entries_cash_account ON income_expense_entries (cash_account_id, entry_date);
CREATE INDEX idx_entries_status ON income_expense_entries (status);
"#;

/// 7 numaralı migration: kullanıcılar ve uygulama ayarları.
///
/// Tasarım kararları:
///
/// * Kullanıcı adı büyük/küçük harf duyarsız benzersizdir; boşluklardan
///   oluşan adlar `CHECK` ile reddedilir.
/// * Şifre **asla** düz metin değildir; `password_hash` Argon2id PHC biçimini
///   taşır. Rol yalnızca `yonetici`/`operator` olabilir.
/// * Kullanıcılar bu tablodan silinebilir (tek istisna); iş kayıtları silinmez.
/// * `app_settings` basit anahtar-değer deposudur. İlk değerler burada
///   yazılır; sonraki açılışlarda kullanıcı değişiklikleri korunur.
const MIGRATION_0007_USERS_AND_SETTINGS: &str = r#"
CREATE TABLE users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT    NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(username)) > 0),
    password_hash TEXT    NOT NULL CHECK (length(password_hash) > 0),
    role          TEXT    NOT NULL CHECK (role IN ('yonetici', 'operator')),
    display_name  TEXT,
    active        INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    last_login_at TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_users_username ON users (username COLLATE NOCASE);

CREATE TABLE app_settings (
    key        TEXT PRIMARY KEY CHECK (length(trim(key)) > 0),
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO app_settings (key, value) VALUES
    ('language', 'tr'),
    ('auto_lock_enabled', '1'),
    ('auto_lock_minutes', '15'),
    ('log_level', 'warn'),
    ('last_auto_backup_at', ''),
    ('backup_daily_keep', '30'),
    ('backup_monthly_keep', '12'),
    ('backup_yearly_keep', '5'),
    ('backup_total_warn_mb', '1000');
"#;

/// Migration geçmişini tutan altyapı tablosu.
const SCHEMA_MIGRATIONS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version    INTEGER PRIMARY KEY NOT NULL,
    name       TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

/// Uygulamanın bildiği en yüksek şema sürümü.
pub fn latest_version() -> i32 {
    MIGRATIONS
        .last()
        .map(|migration| migration.version)
        .unwrap_or(0)
}

/// `schema_migrations` tablosunun var olduğundan emin olur.
fn ensure_migrations_table(connection: &Connection) -> Result<()> {
    connection.execute_batch(SCHEMA_MIGRATIONS_TABLE)?;
    Ok(())
}

/// Uygulanmış migration sürümlerini artan sırada döndürür.
pub fn applied_versions(connection: &Connection) -> Result<Vec<i32>> {
    let mut statement =
        connection.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let rows = statement.query_map([], |row| row.get::<_, i32>(0))?;

    let mut versions = Vec::new();
    for row in rows {
        versions.push(row?);
    }
    Ok(versions)
}

/// Uygulanmış migration kayıtlarını artan sırada döndürür.
pub fn applied_migrations(connection: &Connection) -> Result<Vec<AppliedMigration>> {
    let mut statement = connection
        .prepare("SELECT version, name, applied_at FROM schema_migrations ORDER BY version")?;
    let rows = statement.query_map([], |row| {
        Ok(AppliedMigration {
            version: row.get(0)?,
            name: row.get(1)?,
            applied_at: row.get(2)?,
        })
    })?;

    let mut migrations = Vec::new();
    for row in rows {
        migrations.push(row?);
    }
    Ok(migrations)
}

/// Veritabanının mevcut şema sürümü (uygulanmış en yüksek sürüm).
pub fn schema_version(connection: &Connection) -> Result<i32> {
    Ok(applied_versions(connection)?.into_iter().max().unwrap_or(0))
}

/// Bekleyen migration'ları sırayla uygular ve uygulanan sürümleri döndürür.
///
/// Her migration kendi işlemi (transaction) içinde çalışır. Bu fonksiyon
/// birden fazla kez çağrılabilir; ikinci çağrıda hiçbir şey yapmaz.
pub fn apply_pending(connection: &mut Connection) -> Result<Vec<i32>> {
    ensure_migrations_table(connection)?;

    let already_applied = applied_versions(connection)?;

    // Veritabanı, bu uygulama sürümünden daha yeni bir şema içeriyorsa devam etme.
    let newest = already_applied.last().copied().unwrap_or(0);
    if newest > latest_version() {
        return Err(AppError::SchemaTooNew {
            found: newest,
            supported: latest_version(),
        });
    }

    // Daha yüksek bir sürüm uygulanmışken aradaki bir sürümün kaydı silinmişse
    // migration'ı yeniden çalıştırmak yanıltıcı SQLite hataları üretir. Veriye
    // dokunmadan açık bir zincir hatası döndür.
    for migration in MIGRATIONS {
        if migration.version > newest {
            break;
        }
        if !already_applied.contains(&migration.version) {
            return Err(AppError::MigrationChainBroken {
                missing: migration.version,
            });
        }
    }

    let mut applied_now = Vec::new();

    for migration in MIGRATIONS {
        if already_applied.contains(&migration.version) {
            continue;
        }

        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            params![migration.version, migration.name],
        )?;
        transaction.commit()?;

        applied_now.push(migration.version);
    }

    Ok(applied_now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_numaralari_tek_ve_artan() {
        let mut previous = 0;
        for migration in MIGRATIONS {
            assert!(
                migration.version > previous,
                "migration sürümleri artan olmalı: {} -> {}",
                previous,
                migration.version
            );
            assert!(!migration.name.is_empty(), "migration adı boş olamaz");
            previous = migration.version;
        }
        assert_eq!(latest_version(), previous);
    }

    #[test]
    fn bekleyen_migrationlar_bir_kez_uygulanir() {
        let mut connection = Connection::open_in_memory().expect("bellek içi veritabanı");

        let first_run = apply_pending(&mut connection).expect("ilk uygulama");
        assert_eq!(
            first_run,
            vec![1, 2, 3, 4, 5, 6, 7],
            "ilk çalıştırmada tüm sürümler uygulanmalı"
        );

        let second_run = apply_pending(&mut connection).expect("ikinci uygulama");
        assert!(second_run.is_empty(), "ikinci kez migration uygulanmamalı");

        assert_eq!(schema_version(&connection).unwrap(), latest_version());
        assert_eq!(
            latest_version(),
            7,
            "kullanıcılar ve ayarlar migration'ı eklenmiş olmalı"
        );
    }

    #[test]
    fn cari_hesap_tablosu_ve_benzersiz_kod_kisiti_olusturulur() {
        let mut connection = Connection::open_in_memory().expect("bellek içi veritabanı");
        apply_pending(&mut connection).expect("migration uygulanmalı");

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'customers'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1, "customers tablosu oluşturulmalı");

        let insert = |code: &str| {
            connection.execute(
                "INSERT INTO customers (code, name, kind) VALUES (?1, ?2, 'musteri')",
                params![code, "Deneme"],
            )
        };

        insert("CAR-0001").expect("ilk kayıt eklenebilmeli");
        assert!(
            insert("CAR-0001").is_err(),
            "aynı kod ikinci kez eklenememeli"
        );
        assert!(
            insert("car-0001").is_err(),
            "büyük/küçük harf farkı olan kod da benzersiz sayılmalı"
        );

        // Zorunlu alan ve değer kısıtları veritabanı düzeyinde korunmalı.
        let invalid_kind = connection.execute(
            "INSERT INTO customers (code, name, kind) VALUES ('CAR-0002', 'X', 'gecersiz')",
            [],
        );
        assert!(invalid_kind.is_err(), "geçersiz cari türü yazılamamalı");

        let empty_name = connection.execute(
            "INSERT INTO customers (code, name, kind) VALUES ('CAR-0003', '   ', 'musteri')",
            [],
        );
        assert!(empty_name.is_err(), "boş ad yazılamamalı");

        let invalid_active = connection.execute(
            "INSERT INTO customers (code, name, kind, active) VALUES ('CAR-0004', 'X', 'musteri', 7)",
            [],
        );
        assert!(
            invalid_active.is_err(),
            "geçersiz durum değeri yazılamamalı"
        );
    }

    #[test]
    fn urun_ve_stok_tablolari_ile_kisitlar_olusturulur() {
        let mut connection = Connection::open_in_memory().expect("bellek içi veritabanı");
        apply_pending(&mut connection).expect("migration uygulanmalı");

        for table in ["products", "product_stock_movements"] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "{table} tablosu oluşturulmalı");
        }

        let insert = |code: &str, barcode: Option<&str>| {
            connection.execute(
                "INSERT INTO products (code, barcode, name, unit) VALUES (?1, ?2, 'Deneme', 'adet')",
                params![code, barcode],
            )
        };

        insert("URN-0001", Some("8691234567890")).expect("ilk ürün eklenebilmeli");
        assert!(
            insert("urn-0001", None).is_err(),
            "büyük/küçük harf farkı olan kod da benzersiz sayılmalı"
        );
        assert!(
            insert("URN-0002", Some("8691234567890")).is_err(),
            "aynı barkod ikinci kez eklenememeli"
        );
        insert("URN-0003", None).expect("barkodsuz ürün eklenebilmeli");
        insert("URN-0004", Some("")).expect("boş barkod çoklu olabilmeli");
        insert("URN-0005", Some("")).expect("boş barkod çoklu olabilmeli");

        // Zorunlu alan ve değer kısıtları veritabanı düzeyinde korunmalı.
        assert!(
            connection
                .execute(
                    "INSERT INTO products (code, name, unit) VALUES ('URN-0006', 'X', 'gecersiz')",
                    [],
                )
                .is_err(),
            "geçersiz birim yazılamamalı"
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO products (code, name, unit) VALUES ('URN-0007', '   ', 'adet')",
                    [],
                )
                .is_err(),
            "boş ad yazılamamalı"
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO products (code, name, unit, vat_rate) \
                     VALUES ('URN-0008', 'X', 'adet', 200)",
                    [],
                )
                .is_err(),
            "geçersiz KDV oranı yazılamamalı"
        );

        // Stok hareketi: geçersiz tür ve sıfır miktar reddedilmeli.
        let product_id: i64 = connection
            .query_row(
                "SELECT id FROM products WHERE code = 'URN-0001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            connection
                .execute(
                    "INSERT INTO product_stock_movements \
                     (product_id, movement_type, quantity_delta, movement_date) \
                     VALUES (?1, 'satis', 1000, '2026-10-18')",
                    [product_id],
                )
                .is_err(),
            "geçersiz hareket türü yazılamamalı"
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO product_stock_movements \
                     (product_id, movement_type, quantity_delta, movement_date) \
                     VALUES (?1, 'giris', 0, '2026-10-18')",
                    [product_id],
                )
                .is_err(),
            "sıfır miktarlı hareket yazılamamalı"
        );
    }

    #[test]
    fn fatura_tablolari_kurulur_ve_stok_hareketleri_korunur() {
        let mut connection = Connection::open_in_memory().expect("bellek içi veritabanı");

        // v4 öncesi bir veritabanı simüle edilir: v1-v3 elle uygulanır.
        ensure_migrations_table(&connection).unwrap();
        for migration in MIGRATIONS.iter().filter(|migration| migration.version <= 3) {
            connection
                .execute_batch(migration.sql)
                .expect("v1-v3 uygulanmalı");
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                    params![migration.version, migration.name],
                )
                .unwrap();
        }

        connection
            .execute(
                "INSERT INTO customers (code, name, kind) VALUES ('CAR-0001', 'Deneme', 'musteri')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO products (code, name, unit, current_stock) \
                 VALUES ('URN-0001', 'Vida', 'adet', 4000)",
                [],
            )
            .unwrap();
        let product_id: i64 = connection
            .query_row(
                "SELECT id FROM products WHERE code = 'URN-0001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO product_stock_movements \
                 (product_id, movement_type, quantity_delta, movement_date, description) \
                 VALUES (?1, 'ilk_stok', 5000, '2026-01-01', 'Başlangıç')",
                [product_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO product_stock_movements \
                 (product_id, movement_type, quantity_delta, movement_date) \
                 VALUES (?1, 'cikis', -1000, '2026-02-01')",
                [product_id],
            )
            .unwrap();

        let sum_before: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(quantity_delta), 0) FROM product_stock_movements",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // Yalnızca v4-v7 uygulanmalı.
        let applied = apply_pending(&mut connection).expect("v4 uygulanmalı");
        assert_eq!(applied, vec![4, 5, 6, 7]);
        assert_eq!(schema_version(&connection).unwrap(), 7);

        // Mevcut hareketler **korunmalı**.
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM product_stock_movements", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 2, "eski hareketler korunmalı");
        let sum_after: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(quantity_delta), 0) FROM product_stock_movements",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sum_after, sum_before, "stok toplamı değişmemeli");
        let description: Option<String> = connection
            .query_row(
                "SELECT description FROM product_stock_movements WHERE movement_type = 'ilk_stok'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(description.as_deref(), Some("Başlangıç"));
        let null_invoices: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM product_stock_movements WHERE invoice_id IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            null_invoices, 2,
            "taşınan hareketlerin invoice_id değeri NULL olmalı"
        );

        // Yeni fatura hareket türleri kabul edilmeli, geçersiz tür reddedilmeli.
        connection
            .execute(
                "INSERT INTO product_stock_movements \
                 (product_id, movement_type, quantity_delta, movement_date) \
                 VALUES (?1, 'satis_faturasi', -1000, '2026-03-01')",
                [product_id],
            )
            .expect("satis_faturasi hareketi yazılabilmeli");
        assert!(
            connection
                .execute(
                    "INSERT INTO product_stock_movements \
                     (product_id, movement_type, quantity_delta, movement_date) \
                     VALUES (?1, 'gecersiz', 1000, '2026-03-01')",
                    [product_id],
                )
                .is_err(),
            "geçersiz hareket türü reddedilmeli"
        );

        // Fatura tabloları ve kısıtları.
        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' \
                 AND name IN ('invoices', 'invoice_lines')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 2, "fatura tabloları oluşturulmalı");

        let customer_id: i64 = connection
            .query_row("SELECT id FROM customers", [], |row| row.get(0))
            .unwrap();
        let insert_invoice = |number: &str, invoice_type: &str| {
            connection.execute(
                "INSERT INTO invoices (invoice_number, invoice_type, customer_id, invoice_date) \
                 VALUES (?1, ?2, ?3, '2026-10-18')",
                params![number, invoice_type, customer_id],
            )
        };
        insert_invoice("SF-2026-0001", "satis").expect("fatura eklenebilmeli");
        assert!(
            insert_invoice("sf-2026-0001", "satis").is_err(),
            "büyük/küçük harf farkı olan numara da benzersiz sayılmalı"
        );
        assert!(
            insert_invoice("AF-2026-0001", "gecersiz").is_err(),
            "geçersiz fatura türü yazılamamalı"
        );

        let invoice_id: i64 = connection
            .query_row("SELECT id FROM invoices", [], |row| row.get(0))
            .unwrap();
        assert!(
            connection
                .execute(
                    "INSERT INTO invoice_lines \
                     (invoice_id, product_id, line_no, product_code, product_name, unit, \
                      quantity, unit_price, discount_rate, vat_rate, \
                      line_gross, line_discount, line_net, line_vat, line_total) \
                     VALUES (?1, ?2, 1, 'URN-0001', 'Vida', 'adet', 1000, 1000, 0, 150, \
                             1000, 0, 1000, 1500, 2500)",
                    params![invoice_id, product_id],
                )
                .is_err(),
            "geçersiz KDV oranı yazılamamalı"
        );
    }

    #[test]
    fn kasa_ve_odeme_tablolari_kurulur_mevcut_veri_korunur_ve_paid_amount_eklenir() {
        let mut connection = Connection::open_in_memory().expect("bellek içi veritabanı");

        // v5 öncesi bir veritabanı simüle edilir: v1-v4 elle uygulanır.
        ensure_migrations_table(&connection).unwrap();
        for migration in MIGRATIONS.iter().filter(|migration| migration.version <= 4) {
            connection
                .execute_batch(migration.sql)
                .expect("v1-v4 uygulanmalı");
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                    params![migration.version, migration.name],
                )
                .unwrap();
        }

        // Geçmiş veri: cari, ürün (stoklu), fatura, satır ve stok hareketi.
        connection
            .execute(
                "INSERT INTO customers (code, name, kind) VALUES ('CAR-0001', 'Deneme', 'musteri')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO products (code, name, unit, current_stock) \
                 VALUES ('URN-0001', 'Vida', 'adet', 7500)",
                [],
            )
            .unwrap();
        let customer_id: i64 = connection
            .query_row("SELECT id FROM customers", [], |row| row.get(0))
            .unwrap();
        let product_id: i64 = connection
            .query_row("SELECT id FROM products", [], |row| row.get(0))
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoices \
                 (invoice_number, invoice_type, customer_id, invoice_date, grand_total) \
                 VALUES ('SF-2026-0001', 'satis', ?1, '2026-10-18', 250000)",
                [customer_id],
            )
            .unwrap();
        let invoice_id: i64 = connection
            .query_row("SELECT id FROM invoices", [], |row| row.get(0))
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoice_lines \
                 (invoice_id, product_id, line_no, product_code, product_name, unit, \
                  quantity, unit_price, discount_rate, vat_rate, \
                  line_gross, line_discount, line_net, line_vat, line_total) \
                 VALUES (?1, ?2, 1, 'URN-0001', 'Vida', 'adet', 1000, 2500, 0, 20, \
                         2500, 0, 2500, 500, 3000)",
                params![invoice_id, product_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO product_stock_movements \
                 (product_id, movement_type, quantity_delta, movement_date, description, invoice_id) \
                 VALUES (?1, 'satis_faturasi', -1000, '2026-10-18', 'Satış', ?2)",
                params![product_id, invoice_id],
            )
            .unwrap();

        let stock_before: i64 = connection
            .query_row("SELECT current_stock FROM products", [], |row| row.get(0))
            .unwrap();
        let line_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM invoice_lines", [], |row| row.get(0))
            .unwrap();
        let movement_sum_before: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(quantity_delta), 0) FROM product_stock_movements",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // Yalnızca v5-v7 uygulanmalı.
        let applied = apply_pending(&mut connection).expect("v5 uygulanmalı");
        assert_eq!(applied, vec![5, 6, 7]);
        assert_eq!(schema_version(&connection).unwrap(), 7);

        // Mevcut fatura `paid_amount = 0` almalı.
        let paid: i64 = connection
            .query_row(
                "SELECT paid_amount FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(paid, 0, "mevcut faturaya 0 ödenmiş tutar atanmalı");

        // Geçmiş veri değişmemeli.
        let stock_after: i64 = connection
            .query_row("SELECT current_stock FROM products", [], |row| row.get(0))
            .unwrap();
        assert_eq!(stock_after, stock_before, "güncel stok değişmemeli");
        let line_count_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM invoice_lines", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            line_count_after, line_count_before,
            "fatura satırları korunmalı"
        );
        let movement_sum_after: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(quantity_delta), 0) FROM product_stock_movements",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            movement_sum_after, movement_sum_before,
            "stok hareketleri toplamı korunmalı"
        );

        // Yeni tablolar ve indeksler oluşmalı.
        for table in ["cash_accounts", "payments", "payment_allocations"] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "{table} tablosu oluşturulmalı");
        }

        for index in [
            "idx_cash_accounts_active",
            "idx_payments_customer",
            "idx_payments_cash_account",
            "idx_payments_type_date",
            "idx_payments_status",
            "idx_payment_allocations_payment",
            "idx_payment_allocations_invoice",
            "idx_invoices_paid_amount",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    [index],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "{index} indeksi oluşturulmalı");
        }

        // Kısıtlar veritabanı düzeyinde korunmalı.
        let insert_account = |name: &str, account_type: &str| {
            connection.execute(
                "INSERT INTO cash_accounts (name, account_type) VALUES (?1, ?2)",
                params![name, account_type],
            )
        };
        insert_account("Merkez Kasa", "kasa").expect("kasa hesabı eklenebilmeli");
        assert!(
            insert_account("merkez kasa", "kasa").is_err(),
            "aynı ad (harf farkıyla) ikinci kez eklenememeli"
        );
        assert!(
            insert_account("Geçersiz", "gecersiz").is_err(),
            "geçersiz hesap türü yazılamamalı"
        );
        assert!(
            insert_account("Negatif", "kasa").is_ok(),
            "bakiye değeri sıfır varsayılanı ile eklenebilmeli"
        );

        let account_id: i64 = connection
            .query_row(
                "SELECT id FROM cash_accounts WHERE name = 'Merkez Kasa'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // Ödeme: tutar > 0 ve tür kısıtları.
        assert!(
            connection
                .execute(
                    "INSERT INTO payments \
                     (payment_type, customer_id, cash_account_id, payment_date, amount) \
                     VALUES ('gecersiz', ?1, ?2, '2026-10-18', 100)",
                    params![customer_id, account_id],
                )
                .is_err(),
            "geçersiz ödeme türü yazılamamalı"
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO payments \
                     (payment_type, customer_id, cash_account_id, payment_date, amount) \
                     VALUES ('tahsilat', ?1, ?2, '2026-10-18', 0)",
                    params![customer_id, account_id],
                )
                .is_err(),
            "sıfır tutarlı ödeme yazılamamalı"
        );

        connection
            .execute(
                "INSERT INTO payments \
                 (payment_type, customer_id, cash_account_id, payment_date, amount) \
                 VALUES ('tahsilat', ?1, ?2, '2026-10-18', 10000)",
                params![customer_id, account_id],
            )
            .unwrap();
        let payment_id: i64 = connection
            .query_row("SELECT id FROM payments", [], |row| row.get(0))
            .unwrap();

        // Dağıtım: aynı fatura iki kez aynı ödemeye bağlanamaz.
        connection
            .execute(
                "INSERT INTO payment_allocations (payment_id, invoice_id, amount) VALUES (?1, ?2, 5000)",
                params![payment_id, invoice_id],
            )
            .unwrap();
        assert!(
            connection
                .execute(
                    "INSERT INTO payment_allocations (payment_id, invoice_id, amount) \
                     VALUES (?1, ?2, 5000)",
                    params![payment_id, invoice_id],
                )
                .is_err(),
            "aynı ödeme aynı faturaya iki kez bağlanamamalı"
        );
    }

    #[test]
    fn gelir_gider_tablolari_kurulur_mevcut_veri_korunur() {
        let mut connection = Connection::open_in_memory().expect("bellek içi veritabanı");

        // v6 öncesi bir veritabanı simüle edilir: v1-v5 elle uygulanır.
        ensure_migrations_table(&connection).unwrap();
        for migration in MIGRATIONS.iter().filter(|migration| migration.version <= 5) {
            connection
                .execute_batch(migration.sql)
                .expect("v1-v5 uygulanmalı");
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                    params![migration.version, migration.name],
                )
                .unwrap();
        }

        // Geçmiş veri: cari, ürün/stok, fatura, kasa/banka ve tahsilat.
        connection
            .execute(
                "INSERT INTO customers (code, name, kind) VALUES ('CAR-0001', 'Deneme', 'musteri')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO products (code, name, unit, current_stock) \
                 VALUES ('URN-0001', 'Vida', 'adet', 7500)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO cash_accounts \
                 (name, account_type, opening_balance, current_balance) \
                 VALUES ('Merkez Kasa', 'kasa', 100000, 110000)",
                [],
            )
            .unwrap();
        let customer_id: i64 = connection
            .query_row("SELECT id FROM customers", [], |row| row.get(0))
            .unwrap();
        let account_id: i64 = connection
            .query_row("SELECT id FROM cash_accounts", [], |row| row.get(0))
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoices \
                 (invoice_number, invoice_type, customer_id, invoice_date, grand_total, paid_amount) \
                 VALUES ('SF-2026-0001', 'satis', ?1, '2026-10-18', 250000, 10000)",
                [customer_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO payments \
                 (payment_type, customer_id, cash_account_id, payment_date, amount) \
                 VALUES ('tahsilat', ?1, ?2, '2026-10-18', 10000)",
                params![customer_id, account_id],
            )
            .unwrap();

        let stock_before: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(current_stock), 0) FROM products",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let balances_before: Vec<(i64, i64)> = {
            let mut statement = connection
                .prepare("SELECT opening_balance, current_balance FROM cash_accounts ORDER BY id")
                .unwrap();
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .map(|row| row.unwrap())
                .collect()
        };
        let invoice_before: (i64, i64) = connection
            .query_row(
                "SELECT grand_total, paid_amount FROM invoices WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let payment_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0))
            .unwrap();

        // Yalnızca v6-v7 uygulanmalı.
        let applied = apply_pending(&mut connection).expect("v6 uygulanmalı");
        assert_eq!(applied, vec![6, 7]);
        assert_eq!(schema_version(&connection).unwrap(), 7);

        // Yeni tablolar ve indeksler oluşmalı.
        for table in ["expense_categories", "income_expense_entries"] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "{table} tablosu oluşturulmalı");
        }
        for index in [
            "idx_expense_categories_type_active",
            "idx_expense_categories_name",
            "idx_entries_type_date",
            "idx_entries_category",
            "idx_entries_cash_account",
            "idx_entries_status",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    [index],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "{index} indeksi oluşturulmalı");
        }

        // Mevcut veri **değişmemeli**.
        let stock_after: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(current_stock), 0) FROM products",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stock_after, stock_before, "stok toplamı değişmemeli");
        let balances_after: Vec<(i64, i64)> = {
            let mut statement = connection
                .prepare("SELECT opening_balance, current_balance FROM cash_accounts ORDER BY id")
                .unwrap();
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .map(|row| row.unwrap())
                .collect()
        };
        assert_eq!(
            balances_after, balances_before,
            "kasa/banka bakiyeleri değişmemeli"
        );
        let invoice_after: (i64, i64) = connection
            .query_row(
                "SELECT grand_total, paid_amount FROM invoices WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(invoice_after, invoice_before, "fatura değişmemeli");
        let payment_count_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            payment_count_after, payment_count_before,
            "tahsilat/ödemeler korunmalı"
        );

        // Kategori: tür içinde harf duyarsız benzersizlik ve kısıtlar.
        let insert_category = |name: &str, category_type: &str| {
            connection.execute(
                "INSERT INTO expense_categories (name, category_type) VALUES (?1, ?2)",
                params![name, category_type],
            )
        };
        insert_category("Kira", "gider").expect("gider kategorisi eklenebilmeli");
        insert_category("Kira", "gelir").expect("aynı ad gelir türünde kullanılabilmeli");
        assert!(
            insert_category("kira", "gider").is_err(),
            "aynı türde harf farkı benzersizliği bozmamalı"
        );
        assert!(
            insert_category("Geçersiz", "bilinmeyen").is_err(),
            "geçersiz kategori türü yazılamamalı"
        );
        assert!(
            insert_category("   ", "gider").is_err(),
            "boş kategori adı yazılamamalı"
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO expense_categories (name, category_type, active) \
                     VALUES ('Pasif Kategori', 'gider', 7)",
                    [],
                )
                .is_err(),
            "geçersiz aktiflik değeri yazılamamalı"
        );

        let category_id: i64 = connection
            .query_row(
                "SELECT id FROM expense_categories WHERE name = 'Kira' AND category_type = 'gider'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // Hareket: tutar > 0, tür ve durum kısıtları.
        let insert_entry = |entry_type: &str, amount: i64, status: &str| {
            connection.execute(
                "INSERT INTO income_expense_entries \
                 (entry_type, category_id, cash_account_id, entry_date, amount, status) \
                 VALUES (?1, ?2, ?3, '2026-10-18', ?4, ?5)",
                params![entry_type, category_id, account_id, amount, status],
            )
        };
        insert_entry("gider", 250000, "kesin").expect("gider hareketi eklenebilmeli");
        assert!(
            insert_entry("gider", 0, "kesin").is_err(),
            "sıfır tutarlı hareket yazılamamalı"
        );
        assert!(
            insert_entry("bilinmeyen", 100, "kesin").is_err(),
            "geçersiz hareket türü yazılamamalı"
        );
        assert!(
            insert_entry("gider", 100, "bilinmeyen").is_err(),
            "geçersiz hareket durumu yazılamamalı"
        );
    }

    #[test]
    fn daha_yeni_sema_reddedilir() {
        let mut connection = Connection::open_in_memory().expect("bellek içi veritabanı");
        ensure_migrations_table(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                params![latest_version() + 5, "gelecekten_gelen"],
            )
            .unwrap();

        let error = apply_pending(&mut connection).expect_err("hata bekleniyordu");
        assert!(matches!(error, AppError::SchemaTooNew { .. }));
    }
}
