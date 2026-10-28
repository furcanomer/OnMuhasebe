// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Raporlama iş modeli.
//!
//! Raporlar **salt okunurdur** ve yalnızca mevcut tablolardaki kesinleşmiş
//! kayıtlardan türetilir. Bu modül SQL içermez; veritabanı sorguları
//! [`crate::db`] katmanında, çizim ise [`crate::ui`] katmanında yapılır.
//!
//! ## Ortak kurallar
//!
//! * Tutarlar [`Money`] (kuruş / `i64`), miktarlar [`Quantity`]
//!   (mili-birim / `i64`) olarak tutulur; `f32`/`f64` kullanılmaz.
//! * İptal edilen fatura, tahsilat/ödeme ve gelir/gider kayıtları raporlara
//!   **katılmaz**.
//! * Yüzdeler tam sayıdır; payda sıfırsa `None` döner ve arayüz `—` gösterir.
//! * Tarih aralığı boş bırakılırsa ilgili yönde sınır uygulanmaz. Tarihler
//!   `YYYY-MM-DD` biçimindedir.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};

use crate::cash::{AccountType, CashAccount};
use crate::customer::{Customer, CustomerType, normalize_for_search};
use crate::error::{AppError, Result};
use crate::income_expense::{EntryType, IncomeExpenseEntry, PeriodSummary};
use crate::invoice::InvoiceType;
use crate::money::Money;
use crate::payment::{Payment, PaymentType};
use crate::product::{Product, Unit};
use crate::quantity::Quantity;

/// Gelir/gider trendinde gösterilen ay sayısı.
pub const TREND_MONTHS: usize = 6;

/// Rapor ekranlarındaki alt bölümler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ReportKind {
    /// Tek cari için hesap ekstresi.
    #[default]
    CustomerStatement,
    /// Tüm cariler için bakiye özeti.
    CustomerBalances,
    /// Ürünlerin güncel stok durumu.
    StockStatus,
    /// Satış/alış faturalarının dönem özeti.
    SalesPurchase,
    /// Kasa/banka hareketlerinin dönem özeti.
    CashSummary,
    /// Gelir/gider dönem özeti ve aylık trend.
    IncomeExpense,
}

impl ReportKind {
    /// Menüdeki sıraya göre tüm raporlar.
    pub const ALL: [Self; 6] = [
        Self::CustomerStatement,
        Self::CustomerBalances,
        Self::StockStatus,
        Self::SalesPurchase,
        Self::CashSummary,
        Self::IncomeExpense,
    ];

    /// Sekme başlığı.
    pub fn label(self) -> &'static str {
        match self {
            Self::CustomerStatement => "Cari Ekstre",
            Self::CustomerBalances => "Cari Bakiye Özeti",
            Self::StockStatus => "Stok Durumu",
            Self::SalesPurchase => "Satış/Alış Özeti",
            Self::CashSummary => "Kasa/Banka Hareket Özeti",
            Self::IncomeExpense => "Gelir/Gider Özeti",
        }
    }

    /// Kısa açıklama.
    pub fn description(self) -> &'static str {
        match self {
            Self::CustomerStatement => {
                "Seçili cari için fatura, tahsilat ve ödemelerin tarih sıralı dökümü."
            }
            Self::CustomerBalances => "Tüm carilerin satış, alış, tahsilat ve bakiye özeti.",
            Self::StockStatus => "Ürünlerin güncel stok, minimum stok ve durum bilgisi.",
            Self::SalesPurchase => "Kesinleşmiş satış ve alış faturalarının dönem özeti.",
            Self::CashSummary => "Kasa/banka açılış, hareket ve kapanış özeti.",
            Self::IncomeExpense => "Gelir/gider toplamları, dağılımı ve son altı ay trendi.",
        }
    }

    /// Rapor tarih aralığı kullanıyor mu?
    pub fn uses_date_range(self) -> bool {
        !matches!(self, Self::StockStatus)
    }

    /// Rapor açıldığında tarih aralığı varsayılan olarak bu ay mı?
    pub fn defaults_to_current_month(self) -> bool {
        matches!(
            self,
            Self::SalesPurchase | Self::CashSummary | Self::IncomeExpense
        )
    }
}

/// Tarih aralığını doğrular.
///
/// Boş değerler sınır uygulanmadığı anlamına gelir. Geçersiz biçim veya
/// başlangıcın bitişten sonra olması kullanıcıya gösterilecek mesaja çevrilir.
pub fn validate_date_range(date_from: &str, date_to: &str) -> std::result::Result<(), String> {
    let from = date_from.trim();
    let to = date_to.trim();

    if !from.is_empty() && NaiveDate::parse_from_str(from, "%Y-%m-%d").is_err() {
        return Err("Başlangıç tarihi geçerli değil (YYYY-MM-DD).".to_owned());
    }
    if !to.is_empty() && NaiveDate::parse_from_str(to, "%Y-%m-%d").is_err() {
        return Err("Bitiş tarihi geçerli değil (YYYY-MM-DD).".to_owned());
    }
    if !from.is_empty() && !to.is_empty() && from > to {
        return Err("Başlangıç tarihi bitiş tarihinden sonra olamaz.".to_owned());
    }
    Ok(())
}

/// Cari ekstre hareketinin türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LedgerKind {
    /// Satış faturası: cari borçlanır (borç).
    SalesInvoice,
    /// Alış faturası: cari alacaklanır (alacak).
    PurchaseInvoice,
    /// Tahsilat: cari alacağı kapanır (alacak).
    Collection,
    /// Ödeme: cariye borç ödenir (borç).
    Payment,
}

impl LedgerKind {
    /// Arayüzde gösterilen tür adı.
    pub fn label(self) -> &'static str {
        match self {
            Self::SalesInvoice => "Satış Faturası",
            Self::PurchaseInvoice => "Alış Faturası",
            Self::Collection => "Tahsilat",
            Self::Payment => "Ödeme",
        }
    }

    /// Hareket borç tarafına mı yazılır?
    pub fn is_debit(self) -> bool {
        matches!(self, Self::SalesInvoice | Self::Payment)
    }

    /// Aynı tarihli hareketlerin sırası (fatura önce, ödeme sonra).
    fn order(self) -> u8 {
        match self {
            Self::SalesInvoice => 0,
            Self::PurchaseInvoice => 1,
            Self::Collection => 2,
            Self::Payment => 3,
        }
    }
}

/// Veritabanından okunan, henüz dönem ayrımı yapılmamış ekstre hareketi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerMovement {
    /// Kaynak kaydın kimliği (sıralama için).
    pub id: i64,
    /// Hareket tarihi (`YYYY-MM-DD`).
    pub date: String,
    /// Hareket türü.
    pub kind: LedgerKind,
    /// Belge numarası (faturada dolu, tahsilat/ödemede boş).
    pub document_no: Option<String>,
    /// Açıklama.
    pub description: Option<String>,
    /// Pozitif tutar.
    pub amount: Money,
}

/// Cari ekstre tablosundaki tek satır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerStatementRow {
    /// Hareket tarihi.
    pub date: String,
    /// Hareket türü.
    pub kind: LedgerKind,
    /// Belge numarası.
    pub document_no: Option<String>,
    /// Açıklama.
    pub description: Option<String>,
    /// Borç tutarı.
    pub debit: Money,
    /// Alacak tutarı.
    pub credit: Money,
    /// Satır sonundaki yürüyen bakiye.
    pub balance: Money,
}

/// Cari ekstre raporu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomerStatement {
    /// Cari kimliği.
    pub customer_id: i64,
    /// Cari kodu.
    pub customer_code: String,
    /// Cari adı.
    pub customer_name: String,
    /// Dönem başı bakiyesi (önceki kesin hareketlerden).
    pub opening_balance: Money,
    /// Dönem içindeki hareketler (tarih sıralı).
    pub rows: Vec<CustomerStatementRow>,
    /// Dönem borç toplamı.
    pub total_debit: Money,
    /// Dönem alacak toplamı.
    pub total_credit: Money,
    /// Dönem sonu bakiyesi.
    pub closing_balance: Money,
}

impl CustomerStatement {
    /// Hareketlerden ekstre üretir.
    ///
    /// `date_from` öncesindeki hareketler dönem başı bakiyesine, `date_to`
    /// sonrasındaki hareketler ise hiçbir yere katılmaz. Yürüyen bakiye dönem
    /// başı bakiyesiyle başlar. Tüm hareketler dahil edildiğinde kapanış
    /// bakiyesi [`Customer::balance`] ile aynıdır.
    pub fn from_movements(
        customer: &Customer,
        date_from: &str,
        date_to: &str,
        mut movements: Vec<LedgerMovement>,
    ) -> Self {
        let from = date_from.trim();
        let to = date_to.trim();

        movements.retain(|movement| to.is_empty() || movement.date.as_str() <= to);
        movements.sort_by(|left, right| {
            left.date
                .cmp(&right.date)
                .then_with(|| left.kind.order().cmp(&right.kind.order()))
                .then_with(|| left.id.cmp(&right.id))
        });

        let mut opening = Money::ZERO;
        let mut rows = Vec::new();
        let mut total_debit = Money::ZERO;
        let mut total_credit = Money::ZERO;
        let mut running = Money::ZERO;
        let mut period_started = false;

        for movement in movements {
            if !from.is_empty() && movement.date.as_str() < from {
                opening = signed_balance(opening, movement.kind, movement.amount);
                continue;
            }

            if !period_started {
                // İlk dönem hareketinde yürüyen bakiye dönem başı bakiyesinden başlar.
                running = opening;
                period_started = true;
            }

            let (debit, credit) = if movement.kind.is_debit() {
                (movement.amount, Money::ZERO)
            } else {
                (Money::ZERO, movement.amount)
            };
            running = signed_balance(running, movement.kind, movement.amount);
            total_debit = add_money(total_debit, debit);
            total_credit = add_money(total_credit, credit);

            rows.push(CustomerStatementRow {
                date: movement.date,
                kind: movement.kind,
                document_no: movement.document_no,
                description: movement.description,
                debit,
                credit,
                balance: running,
            });
        }

        let closing = if period_started { running } else { opening };

        Self {
            customer_id: customer.id,
            customer_code: customer.code.clone(),
            customer_name: customer.name.clone(),
            opening_balance: opening,
            rows,
            total_debit,
            total_credit,
            closing_balance: closing,
        }
    }
}

/// Bakiye işaretini türe göre uygular.
fn signed_balance(balance: Money, kind: LedgerKind, amount: Money) -> Money {
    if kind.is_debit() {
        add_money(balance, amount)
    } else {
        Money::from_kurus(balance.kurus().saturating_sub(amount.kurus()))
    }
}

/// Taşma durumunda doygun toplama yapar (mevcut özetlerle aynı yaklaşım).
pub(crate) fn add_money(left: Money, right: Money) -> Money {
    Money::from_kurus(left.kurus().saturating_add(right.kurus()))
}

/// Bakiye özetinde carinin durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomerBalanceStatus {
    /// Bakiye pozitif: cari bize borçlu.
    Borclu,
    /// Bakiye negatif: biz cariye borçluyuz.
    Alacakli,
    /// Bakiye sıfır.
    Kapali,
}

impl CustomerBalanceStatus {
    /// Arayüzde gösterilen durum adı.
    pub fn label(self) -> &'static str {
        match self {
            Self::Borclu => "Borçlu",
            Self::Alacakli => "Alacaklı",
            Self::Kapali => "Kapalı",
        }
    }
}

/// Cari bakiye özetindeki tek satır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerBalanceRow {
    /// Cari kimliği.
    pub customer_id: i64,
    /// Cari kodu.
    pub code: String,
    /// Cari adı.
    pub name: String,
    /// Cari türü.
    pub kind: CustomerType,
    /// Kesin satış toplamı.
    pub sales_total: Money,
    /// Kesin alış toplamı.
    pub purchase_total: Money,
    /// Kesin tahsilat toplamı.
    pub collection_total: Money,
    /// Kesin ödeme toplamı.
    pub payment_total: Money,
    /// Bakiye: satış - alış - tahsilat + ödeme.
    pub balance: Money,
}

impl CustomerBalanceRow {
    /// Satır durumu.
    pub fn status(&self) -> CustomerBalanceStatus {
        if self.balance.kurus() > 0 {
            CustomerBalanceStatus::Borclu
        } else if self.balance.kurus() < 0 {
            CustomerBalanceStatus::Alacakli
        } else {
            CustomerBalanceStatus::Kapali
        }
    }

    /// Türkçe normalize edilmiş arama ölçütüne uyuyor mu?
    pub fn matches_search(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        normalize_for_search(&self.code).contains(needle)
            || normalize_for_search(&self.name).contains(needle)
    }
}

/// Cari bakiye özeti filtreleri.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomerBalanceReportFilter {
    /// Kod veya ad üzerinde aranacak metin.
    pub search: String,
    /// Yalnızca bu cari türü (`None` = tümü).
    pub kind: Option<CustomerType>,
    /// Yalnızca bu durum (`None` = tümü).
    pub active: Option<bool>,
    /// Sıfır bakiyeli carileri gizle.
    pub only_nonzero: bool,
}

/// Bakiye özetinde sıralanabilen kolonlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CustomerBalanceSortColumn {
    /// Cari kodu.
    Code,
    /// Cari adı.
    Name,
    /// Cari türü.
    Kind,
    /// Satış toplamı.
    Sales,
    /// Alış toplamı.
    Purchases,
    /// Tahsilat toplamı.
    Collections,
    /// Ödeme toplamı.
    Payments,
    /// Bakiye (varsayılan: mutlak değere göre azalan).
    #[default]
    Balance,
}

impl CustomerBalanceSortColumn {
    /// Tablo başlığı.
    pub fn label(self) -> &'static str {
        match self {
            Self::Code => "Kod",
            Self::Name => "Cari Adı",
            Self::Kind => "Tür",
            Self::Sales => "Satış",
            Self::Purchases => "Alış",
            Self::Collections => "Tahsilat",
            Self::Payments => "Ödeme",
            Self::Balance => "Bakiye",
        }
    }
}

/// Cari bakiye özeti raporu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomerBalanceReport {
    /// Filtrelere uyan satırlar.
    pub rows: Vec<CustomerBalanceRow>,
    /// Pozitif bakiyelerin toplamı (bize borçlu cariler).
    pub total_receivable: Money,
    /// Negatif bakiyelerin mutlak toplamı (bizim borcumuz).
    pub total_payable: Money,
    /// Net bakiye (alacak - borç).
    pub net_balance: Money,
}

impl CustomerBalanceReport {
    /// Satırlardan rapor üretir; arama ve sıfır filtresi burada uygulanır.
    pub fn from_rows(rows: Vec<CustomerBalanceRow>, filter: &CustomerBalanceReportFilter) -> Self {
        let needle = normalize_for_search(filter.search.trim());
        let mut rows: Vec<_> = rows
            .into_iter()
            .filter(|row| row.matches_search(&needle))
            .filter(|row| !filter.only_nonzero || !row.balance.is_zero())
            .collect();

        let mut report = Self {
            rows: Vec::new(),
            ..Self::default()
        };
        for row in rows.drain(..) {
            if row.balance.kurus() > 0 {
                report.total_receivable = add_money(report.total_receivable, row.balance);
            } else if row.balance.kurus() < 0 {
                report.total_payable = Money::from_kurus(
                    report
                        .total_payable
                        .kurus()
                        .saturating_add(row.balance.kurus().saturating_abs()),
                );
            }
            report.net_balance = add_money(report.net_balance, row.balance);
            report.rows.push(row);
        }

        report.sort_by(CustomerBalanceSortColumn::Balance, false);
        report
    }

    /// Satırları istenen kolona göre sıralar.
    ///
    /// Bakiye kolonunda mutlak değer kullanılır; varsayılan sıralama mutlak
    /// bakiyeye göre azalandır.
    pub fn sort_by(&mut self, column: CustomerBalanceSortColumn, ascending: bool) {
        self.rows.sort_by(|left, right| {
            let ordering = match column {
                CustomerBalanceSortColumn::Code => {
                    normalize_for_search(&left.code).cmp(&normalize_for_search(&right.code))
                }
                CustomerBalanceSortColumn::Name => {
                    normalize_for_search(&left.name).cmp(&normalize_for_search(&right.name))
                }
                CustomerBalanceSortColumn::Kind => left.kind.as_db().cmp(right.kind.as_db()),
                CustomerBalanceSortColumn::Sales => {
                    left.sales_total.kurus().cmp(&right.sales_total.kurus())
                }
                CustomerBalanceSortColumn::Purchases => left
                    .purchase_total
                    .kurus()
                    .cmp(&right.purchase_total.kurus()),
                CustomerBalanceSortColumn::Collections => left
                    .collection_total
                    .kurus()
                    .cmp(&right.collection_total.kurus()),
                CustomerBalanceSortColumn::Payments => {
                    left.payment_total.kurus().cmp(&right.payment_total.kurus())
                }
                CustomerBalanceSortColumn::Balance => left
                    .balance
                    .kurus()
                    .saturating_abs()
                    .cmp(&right.balance.kurus().saturating_abs()),
            };
            let ordering = if ascending {
                ordering
            } else {
                ordering.reverse()
            };
            ordering
                .then_with(|| {
                    normalize_for_search(&left.code).cmp(&normalize_for_search(&right.code))
                })
                .then_with(|| left.customer_id.cmp(&right.customer_id))
        });
    }
}

/// Stok durumu sınıflandırması.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StockStatus {
    /// Stok minimumun üzerinde veya minimum tanımlı değil.
    Yeterli,
    /// Stok pozitif fakat minimumun altında.
    Dusuk,
    /// Stok sıfır.
    Sifir,
    /// Stok negatif.
    Eksi,
}

impl StockStatus {
    /// Filtre listesindeki tüm durumlar.
    pub const ALL: [Self; 4] = [Self::Yeterli, Self::Dusuk, Self::Sifir, Self::Eksi];

    /// Arayüzde gösterilen durum adı.
    pub fn label(self) -> &'static str {
        match self {
            Self::Yeterli => "Yeterli",
            Self::Dusuk => "Düşük",
            Self::Sifir => "Sıfır",
            Self::Eksi => "Eksi",
        }
    }

    /// Miktarlardan durum hesaplar.
    pub fn from_values(current: Quantity, minimum: Quantity) -> Self {
        if current.is_negative() {
            Self::Eksi
        } else if current.is_zero() {
            Self::Sifir
        } else if minimum.is_positive() && current < minimum {
            Self::Dusuk
        } else {
            Self::Yeterli
        }
    }
}

/// Stok raporu filtreleri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockReportFilter {
    /// Kod veya ad üzerinde aranacak metin.
    pub search: String,
    /// Yalnızca bu durum (`None` = tümü).
    pub status: Option<StockStatus>,
    /// Yalnızca aktif ürünleri göster.
    pub only_active: bool,
}

impl Default for StockReportFilter {
    fn default() -> Self {
        Self {
            search: String::new(),
            status: None,
            only_active: true,
        }
    }
}

/// Stok raporundaki tek ürün satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockReportRow {
    /// Ürün kimliği.
    pub product_id: i64,
    /// Ürün kodu.
    pub code: String,
    /// Ürün adı.
    pub name: String,
    /// Ölçü birimi.
    pub unit: Unit,
    /// Güncel stok.
    pub current_stock: Quantity,
    /// Minimum stok.
    pub min_stock: Quantity,
    /// Stok durumu.
    pub status: StockStatus,
    /// Alış fiyatıyla stok değeri.
    pub purchase_value: Money,
    /// Satış fiyatıyla tahmini stok değeri.
    pub sale_value: Money,
}

/// Stok raporunun alt bilgi sayıları.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StockReportStats {
    /// Aktif ürün sayısı.
    pub active_products: usize,
    /// Düşük stoklu aktif ürün sayısı.
    pub low_stock: usize,
    /// Sıfır stoklu aktif ürün sayısı.
    pub zero_stock: usize,
    /// Eksi stoklu aktif ürün sayısı.
    pub negative_stock: usize,
    /// Aktif ürünlerin alış fiyatıyla toplam stok değeri.
    pub purchase_value: Money,
    /// Aktif ürünlerin satış fiyatıyla tahmini stok değeri.
    pub sale_value: Money,
}

/// Stok durumu raporu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StockReport {
    /// Filtrelere uyan ürün satırları.
    pub rows: Vec<StockReportRow>,
    /// Tüm aktif ürünler üzerinden alt bilgi.
    pub stats: StockReportStats,
}

impl StockReport {
    /// Ürünlerden rapor üretir.
    ///
    /// Arama ve durum filtresi yalnızca tabloyu etkiler; alt bilgi sayıları ve
    /// değerler **tüm aktif ürünler** üzerinden hesaplanır. Böylece filtre
    /// değiştikçe toplam işletme stoğu değişmez.
    pub fn from_products(products: &[Product], filter: &StockReportFilter) -> Result<Self> {
        let needle = normalize_for_search(filter.search.trim());
        let mut report = Self::default();

        for product in products {
            let status = StockStatus::from_values(product.current_stock, product.min_stock);
            let purchase_value = product
                .purchase_price
                .checked_mul_quantity(product.current_stock)
                .map_err(|error| {
                    AppError::InvalidRecord(format!(
                        "{} ürününün stok değeri hesaplanamadı: {error}",
                        product.code
                    ))
                })?;
            let sale_value = product
                .sale_price
                .checked_mul_quantity(product.current_stock)
                .map_err(|error| {
                    AppError::InvalidRecord(format!(
                        "{} ürününün tahmini satış değeri hesaplanamadı: {error}",
                        product.code
                    ))
                })?;

            if product.active {
                report.stats.active_products += 1;
                match status {
                    StockStatus::Dusuk => report.stats.low_stock += 1,
                    StockStatus::Sifir => report.stats.zero_stock += 1,
                    StockStatus::Eksi => report.stats.negative_stock += 1,
                    StockStatus::Yeterli => {}
                }
                report.stats.purchase_value =
                    add_money(report.stats.purchase_value, purchase_value);
                report.stats.sale_value = add_money(report.stats.sale_value, sale_value);
            }

            if filter.only_active && !product.active {
                continue;
            }
            if filter.status.is_some_and(|required| status != required) {
                continue;
            }
            if !needle.is_empty()
                && !normalize_for_search(&product.code).contains(&needle)
                && !normalize_for_search(&product.name).contains(&needle)
            {
                continue;
            }

            report.rows.push(StockReportRow {
                product_id: product.id,
                code: product.code.clone(),
                name: product.name.clone(),
                unit: product.unit,
                current_stock: product.current_stock,
                min_stock: product.min_stock,
                status,
                purchase_value,
                sale_value,
            });
        }

        report.rows.sort_by(|left, right| {
            normalize_for_search(&left.code).cmp(&normalize_for_search(&right.code))
        });
        Ok(report)
    }
}

/// Satış/alış özetinde gruplama ölçütü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SalesPurchaseGrouping {
    /// Cari hesaba göre grupla.
    #[default]
    Customer,
    /// Ürüne göre grupla.
    Product,
}

impl SalesPurchaseGrouping {
    /// Seçim listesindeki tüm gruplamalar.
    pub const ALL: [Self; 2] = [Self::Customer, Self::Product];

    /// Arayüzde gösterilen ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Customer => "Cari",
            Self::Product => "Ürün",
        }
    }
}

/// Cari bazlı satış/alış satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerSalesRow {
    /// Cari kimliği.
    pub customer_id: i64,
    /// Cari kodu.
    pub code: String,
    /// Cari adı.
    pub name: String,
    /// Kesin satış faturası sayısı.
    pub sales_count: usize,
    /// Kesin alış faturası sayısı.
    pub purchase_count: usize,
    /// Satış toplamı.
    pub sales_total: Money,
    /// Alış toplamı.
    pub purchase_total: Money,
}

/// Ürün bazlı satış/alış satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductSalesRow {
    /// Ürün kimliği.
    pub product_id: i64,
    /// Ürün kodu.
    pub code: String,
    /// Ürün adı.
    pub name: String,
    /// Ölçü birimi.
    pub unit: Unit,
    /// Satılan miktar.
    pub sold_quantity: Quantity,
    /// Satış tutarı.
    pub sales_total: Money,
    /// Alınan miktar.
    pub purchased_quantity: Quantity,
    /// Alış tutarı.
    pub purchase_total: Money,
}

/// Satış/alış özetinin grup dökümü.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SalesPurchaseBreakdown {
    /// Cari bazlı döküm.
    Customer(Vec<CustomerSalesRow>),
    /// Ürün bazlı döküm.
    Product(Vec<ProductSalesRow>),
}

impl SalesPurchaseBreakdown {
    /// Döküm satır sayısı.
    pub fn len(&self) -> usize {
        match self {
            Self::Customer(rows) => rows.len(),
            Self::Product(rows) => rows.len(),
        }
    }

    /// Döküm boş mu?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Satış/alış dönem özeti.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SalesPurchaseReport {
    /// Kesin satış faturası sayısı.
    pub sales_count: usize,
    /// Satış toplamı.
    pub sales_total: Money,
    /// Kesin alış faturası sayısı.
    pub purchase_count: usize,
    /// Alış toplamı.
    pub purchase_total: Money,
    /// Grup dökümü.
    pub breakdown: SalesPurchaseBreakdown,
}

impl Default for SalesPurchaseBreakdown {
    fn default() -> Self {
        Self::Customer(Vec::new())
    }
}

impl SalesPurchaseReport {
    /// Net (satış - alış).
    pub fn net(&self) -> Money {
        Money::from_kurus(
            self.sales_total
                .kurus()
                .saturating_sub(self.purchase_total.kurus()),
        )
    }

    /// Toplam kesin fatura sayısı.
    pub fn invoice_count(&self) -> usize {
        self.sales_count + self.purchase_count
    }
}

/// Kasa/banka hareket türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CashMovementKind {
    /// Tahsilat.
    Collection,
    /// Ödeme.
    Payment,
    /// Gelir.
    Income,
    /// Gider.
    Expense,
}

impl CashMovementKind {
    /// Arayüzde gösterilen ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Collection => "Tahsilat",
            Self::Payment => "Ödeme",
            Self::Income => "Gelir",
            Self::Expense => "Gider",
        }
    }

    /// Hesap bakiyesine işaretli etki.
    pub fn cash_effect(self, amount: Money) -> Money {
        match self {
            Self::Collection | Self::Income => amount,
            Self::Payment | Self::Expense => Money::from_kurus(amount.kurus().saturating_neg()),
        }
    }

    /// Aynı tarihli hareketlerin sırası.
    fn order(self) -> u8 {
        match self {
            Self::Collection => 0,
            Self::Payment => 1,
            Self::Income => 2,
            Self::Expense => 3,
        }
    }
}

/// Kasa raporundaki birleşik hareket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashReportMovement {
    /// Kaynak kayıt kimliği.
    pub id: i64,
    /// Hareket türü.
    pub kind: CashMovementKind,
    /// Kasa/banka hesabı kimliği.
    pub account_id: i64,
    /// Kasa/banka hesabı adı.
    pub account_name: String,
    /// Hareket tarihi.
    pub date: String,
    /// Cari veya kategori adı.
    pub party: String,
    /// Açıklama.
    pub description: Option<String>,
    /// Pozitif tutar.
    pub amount: Money,
}

impl CashReportMovement {
    /// Tahsilat/ödeme kaydından hareket üretir.
    pub fn from_payment(payment: &Payment) -> Self {
        Self {
            id: payment.id,
            kind: match payment.payment_type {
                PaymentType::Tahsilat => CashMovementKind::Collection,
                PaymentType::Odeme => CashMovementKind::Payment,
            },
            account_id: payment.cash_account_id,
            account_name: payment.cash_account_name.clone(),
            date: payment.payment_date.clone(),
            party: format!("{} — {}", payment.customer_code, payment.customer_name),
            description: payment.description.clone(),
            amount: payment.amount,
        }
    }

    /// Gelir/gider kaydından hareket üretir.
    pub fn from_entry(entry: &IncomeExpenseEntry) -> Self {
        Self {
            id: entry.id,
            kind: match entry.entry_type {
                EntryType::Gelir => CashMovementKind::Income,
                EntryType::Gider => CashMovementKind::Expense,
            },
            account_id: entry.cash_account_id,
            account_name: entry.cash_account_name.clone(),
            date: entry.entry_date.clone(),
            party: entry.category_name.clone(),
            description: entry.description.clone(),
            amount: entry.amount,
        }
    }
}

/// Kasa/banka hesabı bazlı rapor satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashReportAccountRow {
    /// Hesap kimliği.
    pub account_id: i64,
    /// Hesap adı.
    pub account_name: String,
    /// Hesap türü.
    pub account_type: AccountType,
    /// Dönem başı bakiyesi.
    pub opening_balance: Money,
    /// Tahsilat toplamı.
    pub collections: Money,
    /// Ödeme toplamı.
    pub payments: Money,
    /// Gelir toplamı.
    pub incomes: Money,
    /// Gider toplamı.
    pub expenses: Money,
    /// Dönem sonu bakiyesi.
    pub closing_balance: Money,
}

impl CashReportAccountRow {
    /// Dönem net etkisi.
    pub fn net(&self) -> Money {
        Money::from_kurus(
            self.collections
                .kurus()
                .saturating_sub(self.payments.kurus())
                .saturating_add(self.incomes.kurus())
                .saturating_sub(self.expenses.kurus()),
        )
    }

    /// Kapanış bakiyesini dönem hareketlerinden yeniden hesaplar.
    fn recompute_closing(&mut self) {
        self.closing_balance = add_money(self.opening_balance, self.net());
    }
}

/// Kasa/banka hareket özeti raporu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CashMovementReport {
    /// Dönem başı toplam bakiye.
    pub opening_balance: Money,
    /// Tahsilat toplamı.
    pub collections: Money,
    /// Ödeme toplamı.
    pub payments: Money,
    /// Gelir toplamı.
    pub incomes: Money,
    /// Gider toplamı.
    pub expenses: Money,
    /// Dönem sonu toplam bakiye.
    pub closing_balance: Money,
    /// Hesap bazlı döküm.
    pub accounts: Vec<CashReportAccountRow>,
    /// Dönem içindeki birleşik hareketler (en yeni önce).
    pub movements: Vec<CashReportMovement>,
}

impl CashMovementReport {
    /// Hesaplardan ve kesin hareketlerden rapor üretir.
    ///
    /// Açılış bakiyesi, hesabın tanımlı açılış bakiyesine dönem öncesindeki
    /// kesin hareketlerin net etkisi eklenerek bulunur. Kapanış bakiyesi
    /// dönem hareketlerinin net etkisiyle hesaplanır.
    pub fn from_accounts(
        accounts: &[CashAccount],
        movements: Vec<CashReportMovement>,
        date_from: &str,
        date_to: &str,
    ) -> Self {
        let from = date_from.trim();
        let to = date_to.trim();

        let mut rows: Vec<CashReportAccountRow> = accounts
            .iter()
            .map(|account| CashReportAccountRow {
                account_id: account.id,
                account_name: account.name.clone(),
                account_type: account.account_type,
                opening_balance: account.opening_balance,
                collections: Money::ZERO,
                payments: Money::ZERO,
                incomes: Money::ZERO,
                expenses: Money::ZERO,
                closing_balance: account.opening_balance,
            })
            .collect();
        let mut index: HashMap<i64, usize> = rows
            .iter()
            .enumerate()
            .map(|(index, row)| (row.account_id, index))
            .collect();

        let mut period_movements = Vec::new();
        for movement in movements {
            if !to.is_empty() && movement.date.as_str() > to {
                continue;
            }
            let Some(&row_index) = index.get(&movement.account_id) else {
                continue;
            };

            if !from.is_empty() && movement.date.as_str() < from {
                let effect = movement.kind.cash_effect(movement.amount);
                rows[row_index].opening_balance =
                    add_money(rows[row_index].opening_balance, effect);
                continue;
            }

            let row = &mut rows[row_index];
            match movement.kind {
                CashMovementKind::Collection => {
                    row.collections = add_money(row.collections, movement.amount);
                }
                CashMovementKind::Payment => {
                    row.payments = add_money(row.payments, movement.amount);
                }
                CashMovementKind::Income => {
                    row.incomes = add_money(row.incomes, movement.amount);
                }
                CashMovementKind::Expense => {
                    row.expenses = add_money(row.expenses, movement.amount);
                }
            }
            period_movements.push(movement);
        }

        for row in &mut rows {
            row.recompute_closing();
        }
        rows.sort_by(|left, right| left.account_name.cmp(&right.account_name));
        // Sıralamadan sonra indeks artık gerekli değil; kullanılmayan uyarısını önle.
        index.clear();

        period_movements.sort_by(|left, right| {
            right
                .date
                .cmp(&left.date)
                .then_with(|| left.kind.order().cmp(&right.kind.order()))
                .then_with(|| right.id.cmp(&left.id))
        });

        let mut report = Self {
            accounts: rows,
            movements: period_movements,
            ..Self::default()
        };
        for account in &report.accounts {
            report.opening_balance = add_money(report.opening_balance, account.opening_balance);
            report.collections = add_money(report.collections, account.collections);
            report.payments = add_money(report.payments, account.payments);
            report.incomes = add_money(report.incomes, account.incomes);
            report.expenses = add_money(report.expenses, account.expenses);
        }
        report.closing_balance = Money::from_kurus(
            report
                .opening_balance
                .kurus()
                .saturating_add(report.collections.kurus())
                .saturating_sub(report.payments.kurus())
                .saturating_add(report.incomes.kurus())
                .saturating_sub(report.expenses.kurus()),
        );
        report
    }

    /// Dönemde hiç hareket yok mu?
    pub fn has_movements(&self) -> bool {
        !self.movements.is_empty()
    }
}

/// Gelir/gider aylık trend satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthlyIncomeExpense {
    /// Yıl.
    pub year: i32,
    /// Ay (1-12).
    pub month: u32,
    /// Kesin gelir toplamı.
    pub income_total: Money,
    /// Kesin gider toplamı.
    pub expense_total: Money,
}

impl MonthlyIncomeExpense {
    /// Net (gelir - gider).
    pub fn net(&self) -> Money {
        Money::from_kurus(
            self.income_total
                .kurus()
                .saturating_sub(self.expense_total.kurus()),
        )
    }

    /// `Ekim 2026` biçiminde etiket.
    pub fn label(&self) -> String {
        format!("{} {}", month_name(self.month), self.year)
    }
}

/// Gelir/gider raporu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IncomeExpenseReport {
    /// Seçili dönemin özeti.
    pub summary: PeriodSummary,
    /// Seçili dönem sonundan geriye altı aylık trend (eskiden yeniye).
    pub trend: Vec<MonthlyIncomeExpense>,
}

impl IncomeExpenseReport {
    /// Seçili dönem verilerinden rapor üretir.
    pub fn from_entries(
        date_from: &str,
        date_to: &str,
        period_entries: &[IncomeExpenseEntry],
        trend_entries: &[IncomeExpenseEntry],
        trend_end: NaiveDate,
    ) -> Self {
        Self {
            summary: PeriodSummary::from_entries(date_from, date_to, period_entries),
            trend: monthly_trend(trend_end, TREND_MONTHS, trend_entries),
        }
    }
}

/// Yüzdeyi arayüzde gösterilecek biçime getirir.
///
/// Payda sıfır olduğunda [`None`] döner ve arayüz `—` gösterir.
pub fn percentage_label(percentage: Option<i64>) -> String {
    match percentage {
        Some(value) => format!("%{value}"),
        None => "—".to_owned(),
    }
}

/// Türkçe ay adı (1 = Ocak).
pub fn month_name(month: u32) -> &'static str {
    const MONTHS: [&str; 12] = [
        "Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim",
        "Kasım", "Aralık",
    ];
    MONTHS
        .get(month.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("")
}

/// Verilen ayın ilk günü.
pub fn month_start(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, 1, 1).expect("1 Ocak her zaman geçerli"))
}

/// Verilen tarihten geriye `count` aylık (yıl, ay) listesi; eskiden yeniye.
pub fn last_months(end: NaiveDate, count: usize) -> Vec<(i32, u32)> {
    let mut result = Vec::with_capacity(count);
    let mut year = end.year();
    let mut month = end.month();
    for _ in 0..count {
        result.push((year, month));
        if month == 1 {
            month = 12;
            year -= 1;
        } else {
            month -= 1;
        }
    }
    result.reverse();
    result
}

/// Trend için sorgu başlangıç tarihi (`YYYY-MM-DD`).
pub fn trend_start_date(end: NaiveDate, count: usize) -> String {
    let months = last_months(end, count);
    let (year, month) = months.first().copied().unwrap_or((end.year(), end.month()));
    month_start(year, month).format("%Y-%m-%d").to_string()
}

/// Kayıtlardan aylık trend üretir; eksik aylar sıfırla doldurulur.
pub fn monthly_trend(
    end: NaiveDate,
    count: usize,
    entries: &[IncomeExpenseEntry],
) -> Vec<MonthlyIncomeExpense> {
    let months = last_months(end, count);
    let mut trend: Vec<MonthlyIncomeExpense> = months
        .into_iter()
        .map(|(year, month)| MonthlyIncomeExpense {
            year,
            month,
            income_total: Money::ZERO,
            expense_total: Money::ZERO,
        })
        .collect();
    let index: HashMap<(i32, u32), usize> = trend
        .iter()
        .enumerate()
        .map(|(index, row)| ((row.year, row.month), index))
        .collect();

    for entry in entries {
        if entry.is_cancelled() {
            continue;
        }
        let Some(date) = parse_iso_date(&entry.entry_date) else {
            continue;
        };
        let Some(&row_index) = index.get(&(date.year(), date.month())) else {
            continue;
        };
        let row = &mut trend[row_index];
        match entry.entry_type {
            EntryType::Gelir => {
                row.income_total = add_money(row.income_total, entry.amount);
            }
            EntryType::Gider => {
                row.expense_total = add_money(row.expense_total, entry.amount);
            }
        }
    }

    trend
}

/// `YYYY-MM-DD` tarihini çözer.
pub fn parse_iso_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok()
}

/// Fatura türünü ekstre hareket türüne çevirir.
pub fn ledger_kind_for_invoice(invoice_type: InvoiceType) -> LedgerKind {
    match invoice_type {
        InvoiceType::Sales => LedgerKind::SalesInvoice,
        InvoiceType::Purchase => LedgerKind::PurchaseInvoice,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn customer() -> Customer {
        Customer {
            id: 7,
            code: "CAR-0007".to_owned(),
            name: "Örnek Cari".to_owned(),
            kind: CustomerType::Customer,
            phone: None,
            email: None,
            address: None,
            tax_office: None,
            tax_number: None,
            national_id: None,
            contact_person: None,
            notes: None,
            active: true,
            created_at: String::new(),
            updated_at: String::new(),
            sales_total: Money::ZERO,
            purchase_total: Money::ZERO,
            invoice_count: 0,
            collection_total: Money::ZERO,
            payment_total: Money::ZERO,
            payment_count: 0,
        }
    }

    fn product(id: i64, code: &str, current: i64, minimum: i64, active: bool) -> Product {
        Product {
            id,
            code: code.to_owned(),
            barcode: None,
            name: format!("Ürün {code}"),
            unit: Unit::Adet,
            category: None,
            purchase_price: Money::from_kurus(1_000),
            sale_price: Money::from_kurus(1_500),
            vat_rate: 20,
            min_stock: Quantity::from_milli(minimum),
            current_stock: Quantity::from_milli(current),
            notes: None,
            active,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn entry(
        id: i64,
        date: &str,
        entry_type: EntryType,
        amount: i64,
        cancelled: bool,
    ) -> IncomeExpenseEntry {
        IncomeExpenseEntry {
            id,
            entry_type,
            category_id: 1,
            category_name: "Kategori".to_owned(),
            cash_account_id: 1,
            cash_account_name: "Kasa".to_owned(),
            entry_date: date.to_owned(),
            amount: Money::from_kurus(amount),
            description: None,
            document_ref: None,
            status: if cancelled {
                crate::income_expense::EntryStatus::Iptal
            } else {
                crate::income_expense::EntryStatus::Kesin
            },
            cancelled_at: None,
            cancel_reason: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn tarih_araligi_dogrulanir() {
        assert!(validate_date_range("", "").is_ok());
        assert!(validate_date_range("2026-10-01", "2026-10-31").is_ok());
        assert!(validate_date_range("2026-10-31", "2026-10-01").is_err());
        assert!(validate_date_range("31.10.2026", "").is_err());
    }

    #[test]
    fn ekstre_donem_basi_ve_yuruyen_bakiyeyi_hesaplar() {
        let customer = customer();
        let movements = vec![
            LedgerMovement {
                id: 1,
                date: "2026-09-10".to_owned(),
                kind: LedgerKind::SalesInvoice,
                document_no: Some("SAT-1".to_owned()),
                description: None,
                amount: Money::from_kurus(10_000),
            },
            LedgerMovement {
                id: 2,
                date: "2026-10-02".to_owned(),
                kind: LedgerKind::Collection,
                document_no: None,
                description: None,
                amount: Money::from_kurus(3_000),
            },
            LedgerMovement {
                id: 3,
                date: "2026-10-03".to_owned(),
                kind: LedgerKind::Payment,
                document_no: None,
                description: None,
                amount: Money::from_kurus(1_000),
            },
        ];

        let statement =
            CustomerStatement::from_movements(&customer, "2026-10-01", "2026-10-31", movements);
        assert_eq!(statement.opening_balance.kurus(), 10_000);
        assert_eq!(statement.total_debit.kurus(), 1_000);
        assert_eq!(statement.total_credit.kurus(), 3_000);
        assert_eq!(statement.closing_balance.kurus(), 8_000);
        assert_eq!(statement.rows.len(), 2);
        assert_eq!(statement.rows[1].balance.kurus(), 8_000);
    }

    #[test]
    fn bakiye_durumu_isarete_gore_belirlenir() {
        let row = CustomerBalanceRow {
            customer_id: 1,
            code: "A".to_owned(),
            name: "A".to_owned(),
            kind: CustomerType::Customer,
            sales_total: Money::from_kurus(100),
            purchase_total: Money::ZERO,
            collection_total: Money::ZERO,
            payment_total: Money::ZERO,
            balance: Money::from_kurus(100),
        };
        assert_eq!(row.status(), CustomerBalanceStatus::Borclu);
        let mut negative = row.clone();
        negative.balance = Money::from_kurus(-100);
        assert_eq!(negative.status(), CustomerBalanceStatus::Alacakli);
        let mut zero = row;
        zero.balance = Money::ZERO;
        assert_eq!(zero.status(), CustomerBalanceStatus::Kapali);
    }

    #[test]
    fn stok_durum_esikleri_dogrudur() {
        assert_eq!(
            StockStatus::from_values(Quantity::from_milli(2_000), Quantity::from_milli(1_000)),
            StockStatus::Yeterli
        );
        assert_eq!(
            StockStatus::from_values(Quantity::from_milli(500), Quantity::from_milli(1_000)),
            StockStatus::Dusuk
        );
        assert_eq!(
            StockStatus::from_values(Quantity::ZERO, Quantity::from_milli(1_000)),
            StockStatus::Sifir
        );
        assert_eq!(
            StockStatus::from_values(Quantity::from_milli(-1), Quantity::ZERO),
            StockStatus::Eksi
        );
    }

    #[test]
    fn stok_alt_bilgisi_filtreden_bagimsizdir() {
        let products = vec![
            product(1, "A", 2_000, 1_000, true),
            product(2, "B", 500, 1_000, true),
            product(3, "C", 0, 0, false),
        ];
        let report = StockReport::from_products(
            &products,
            &StockReportFilter {
                search: "A".to_owned(),
                status: None,
                only_active: true,
            },
        )
        .expect("rapor üretilmeli");

        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.stats.active_products, 2);
        assert_eq!(report.stats.low_stock, 1);
        assert_eq!(report.stats.purchase_value.kurus(), 2_500);
        assert_eq!(report.stats.sale_value.kurus(), 3_750);
    }

    #[test]
    fn bakiye_ozeti_mutlak_bakiyeye_gore_siralanir() {
        let rows = vec![
            CustomerBalanceRow {
                customer_id: 1,
                code: "A".to_owned(),
                name: "A".to_owned(),
                kind: CustomerType::Customer,
                sales_total: Money::ZERO,
                purchase_total: Money::ZERO,
                collection_total: Money::ZERO,
                payment_total: Money::ZERO,
                balance: Money::from_kurus(100),
            },
            CustomerBalanceRow {
                customer_id: 2,
                code: "B".to_owned(),
                name: "B".to_owned(),
                kind: CustomerType::Customer,
                sales_total: Money::ZERO,
                purchase_total: Money::ZERO,
                collection_total: Money::ZERO,
                payment_total: Money::ZERO,
                balance: Money::from_kurus(-500),
            },
        ];
        let report =
            CustomerBalanceReport::from_rows(rows, &CustomerBalanceReportFilter::default());
        assert_eq!(report.rows[0].customer_id, 2);
        assert_eq!(report.total_payable.kurus(), 500);
        assert_eq!(report.net_balance.kurus(), -400);
    }

    #[test]
    fn aylik_trend_alti_ay_ve_iptalleri_dogru_ele_alir() {
        let end = NaiveDate::from_ymd_opt(2026, 10, 20).expect("tarih");
        let entries = vec![
            entry(1, "2026-05-03", EntryType::Gelir, 1_000, false),
            entry(2, "2026-07-04", EntryType::Gider, 400, false),
            entry(3, "2026-07-05", EntryType::Gelir, 9_999, true),
        ];
        let trend = monthly_trend(end, TREND_MONTHS, &entries);

        assert_eq!(trend.len(), 6);
        assert_eq!(trend[0].label(), "Mayıs 2026");
        assert_eq!(trend[0].income_total.kurus(), 1_000);
        assert_eq!(trend[2].expense_total.kurus(), 400);
        assert_eq!(trend[5].label(), "Ekim 2026");
    }
}
