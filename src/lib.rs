// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! # Ön Muhasebe
//!
//! Windows üzerinde **yerel (native) masaüstü uygulaması** olarak çalışan ön
//! muhasebe programının çekirdek kütüphanesi.
//!
//! ## Katmanlar
//!
//! | Modül      | Sorumluluk                                                        |
//! |------------|-------------------------------------------------------------------|
//! | [`config`]   | Uygulama veri dizinini ve veritabanı yolunu çözer                |
//! | [`db`]       | SQLite bağlantısı, şema sürümleme, migration ve cari veri erişimi |
//! | [`money`]    | Kuruş cinsinden tam sayı tabanlı para tipi (iş mantığı temeli)   |
//! | [`quantity`] | Mili-birim cinsinden tam sayı tabanlı stok miktarı tipi           |
//! | [`customer`] | Cari hesap iş modeli ve doğrulama kuralları                       |
//! | [`product`]  | Ürün ve stok iş modeli, hareket kuralları                         |
//! | [`invoice`]  | Alış/satış faturası iş modeli, hesaplama ve doğrulama             |
//! | [`cash`]     | Kasa/banka hesabı iş modeli ve doğrulama                          |
//! | [`payment`]  | Tahsilat/ödeme iş modeli, fatura dağıtımı ve doğrulama            |
//! | [`income_expense`] | Gelir/gider kategorileri ve hareketleri iş modeli           |
//! | [`report`]   | Salt okunur rapor hesaplamaları ve filtreleri                    |
//! | [`ui`]       | egui/eframe tabanlı arayüz katmanı                               |
//! | [`app`]      | Yukarıdaki katmanları bir araya getiren uygulama durumu           |
//! | [`error`]    | Tüm katmanların kullandığı ortak hata tipi                       |
//!
//! ## Bağımlılık yönü
//!
//! ```text
//! main  ->  app  ->  ui
//!                 ->  db       ->  error
//!                 ->  config
//!                 ->  money
//!                 ->  quantity
//!                 ->  customer ->  money / error
//!                 ->  product  ->  money / quantity / customer / error
//!                 ->  invoice  ->  money / quantity / product / customer / error
//!                 ->  cash     ->  money / error
//!                 ->  payment  ->  money / customer / invoice / error
//!                 ->  income_expense -> money / customer / error
//!                 ->  report   ->  money / quantity / customer / product / invoice
//!                                  / payment / cash / income_expense / error
//! ```
//!
//! Arayüz katmanı veritabanına doğrudan erişmez; tüm veri erişimi [`db`]
//! üzerinden ve [`app::OnMuhasebeApp`] aracılığıyla yapılır.
//!
//! ## Kapsam notu
//!
//! Bu sürümde **cari hesap**, **ürün/stok**, **alış/satış faturası**,
//! **kasa/banka + tahsilat/ödeme**, **gelir/gider** ve **raporlama**
//! modülleri çalışmaktadır. Raporlar salt okunurdur; dışa aktarma (CSV/Excel/
//! PDF) henüz desteklenmiyor.

pub mod app;
pub mod auth;
pub mod backup;
pub mod cash;
pub mod config;
pub mod csv;
pub mod customer;
pub mod db;
pub mod error;
pub mod icon;
pub mod income_expense;
pub mod invoice;
pub mod logging;
pub mod money;
pub mod onboarding;
pub mod payment;
pub mod product;
pub mod quantity;
pub mod report;
pub mod restore_journal;
pub mod settings;
pub mod ui;

/// Uygulamanın görünen adı (pencere başlığı ve "Hakkında" penceresi).
pub const APP_NAME: &str = "Ön Muhasebe";

/// Cargo tarafından üretilen sürüm bilgisi.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Derleme sırasında verilebilen Git kısa hash'i; proje Git deposu değilse
/// "bilinmiyor" gösterilir.
pub const GIT_HASH: &str = match option_env!("GIT_HASH") {
    Some(hash) => hash,
    None => "bilinmiyor",
};

/// Kullanıcının yerel uygulama verisi dizininde oluşturulan veritabanı dosyası.
pub const DATABASE_FILE_NAME: &str = "on_muhasebe.db";
