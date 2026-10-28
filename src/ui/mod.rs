// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Arayüz katmanı (egui / eframe).
//!
//! Bu katman veritabanına doğrudan erişmez; ihtiyaç duyduğu tüm veriyi
//! [`crate::app::OnMuhasebeApp`] üzerinden okur. Böylece SQL ve şema bilgisi
//! arayüz koduna sızmaz ve arayüz test edilebilir kalır.
//!
//! | Modül                   | Sorumluluk                                          |
//! |-------------------------|-----------------------------------------------------|
//! | [`theme`]               | Renk paleti, ölçüler ve egui stil ayarları          |
//! | [`widgets`]             | Ekranlar arasında paylaşılan küçük bileşenler       |
//! | [`format`]              | Tarih ve boyut biçimlendirme (saf fonksiyonlar)     |
//! | [`layout`]              | Panellerin yerleşimi ve görünüm yönlendirmesi       |
//! | [`navigation`]          | Sol gezinme menüsü                                  |
//! | [`overview`]            | "Genel Bakış" ekranı                                |
//! | [`customers`]           | Cari hesap listesi, formu ve ayrıntısı              |
//! | [`products`]            | Ürün listesi, formu, ayrıntısı ve stok hareketleri  |
//! | [`invoices`]            | Alış/satış fatura listesi, formu ve ayrıntısı       |
//! | [`cash`]                | Kasa/banka hesapları ve tahsilat/ödeme hareketleri  |
//! | [`income_expense`]      | Gelir/gider hareketleri, kategoriler ve dönem özeti |
//! | [`reports`]             | Altı salt okunur rapor ve filtreleri                |
//! | [`placeholder`]         | Geliştirilmemiş bölümlerin bilgilendirme ekranı     |

pub mod auth;
pub mod cash;
pub mod customers;
pub mod format;
pub mod income_expense;
pub mod invoices;
pub mod layout;
pub mod navigation;
pub mod overlays;
pub mod overview;
pub mod placeholder;
pub mod products;
pub mod reports;
pub mod settings;
pub mod theme;
pub mod transfer;
pub mod virtual_list;
pub mod widgets;
