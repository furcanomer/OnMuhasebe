// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Global arama: cari, ürün, fatura ve kasa/banka hesaplarında önek araması.
//!
//! Arama **SQL tarafında** `LIKE 'önek%'` ve `LIMIT` ile yapılır. Böylece
//! yüz binlerce kayıt belleğe alınmadan yalnızca en fazla beş sonuç okunur.
//! Türkçe karakterler için sınırlı sayıda yazım varyantı üretilir.

use std::collections::HashSet;

use crate::customer::normalize_for_search;
use crate::error::Result;
use crate::invoice::InvoiceType;

use super::Database;

/// Arama sonucunun ait olduğu grup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchGroup {
    /// Cari hesaplar.
    Customers,
    /// Ürünler.
    Products,
    /// Faturalar.
    Invoices,
    /// Kasa/banka hesapları.
    CashAccounts,
}

impl SearchGroup {
    /// Sonuç ekranındaki grup başlığı.
    pub fn label(self) -> &'static str {
        match self {
            Self::Customers => "Cari Hesaplar",
            Self::Products => "Ürünler",
            Self::Invoices => "Faturalar",
            Self::CashAccounts => "Kasa / Banka",
        }
    }
}

/// Arama sonucunun türü ve hedef kaydın kimliği.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchKind {
    /// Cari hesap.
    Customer,
    /// Ürün.
    Product,
    /// Fatura (türüyle birlikte).
    Invoice(InvoiceType),
    /// Kasa/banka hesabı.
    CashAccount,
}

impl SearchKind {
    /// Sonucun grubu.
    pub fn group(self) -> SearchGroup {
        match self {
            Self::Customer => SearchGroup::Customers,
            Self::Product => SearchGroup::Products,
            Self::Invoice(_) => SearchGroup::Invoices,
            Self::CashAccount => SearchGroup::CashAccounts,
        }
    }
}

/// Arayüzde gösterilecek tek bir arama sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// Tür ve hedef kimliği.
    pub kind: SearchKind,
    /// Hedef kaydın birincil anahtarı.
    pub id: i64,
    /// Ana metin (cari adı, ürün adı, fatura numarası, hesap adı).
    pub title: String,
    /// İkincil açıklama (kod, tarih, tür).
    pub subtitle: String,
}

impl Database {
    /// Dört grupta önek araması yapar; her gruptan en fazla `per_group`
    /// (varsayılan 5), toplamda en fazla 20 sonuç döner.
    pub fn global_search(&self, query: &str) -> Result<Vec<SearchHit>> {
        let normalized = normalize_for_search(query.trim());
        if normalized.is_empty() {
            return Ok(Vec::new());
        }

        let variants = search_like_variants(&normalized);
        if variants.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        results.extend(self.search_customers(&variants, 5)?);
        results.extend(self.search_products(&variants, 5)?);
        results.extend(self.search_invoices(&variants, 5)?);
        results.extend(self.search_cash_accounts(&variants, 5)?);
        results.truncate(20);
        Ok(results)
    }

    fn search_customers(&self, variants: &[String], limit: usize) -> Result<Vec<SearchHit>> {
        let mut hits = self.search_customers_by("c.code", variants, limit)?;
        hits.extend(self.search_customers_by("c.name", variants, limit)?);
        Ok(merge_hits(hits, limit))
    }

    fn search_customers_by(
        &self,
        column: &str,
        variants: &[String],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let sql = format!(
            "SELECT c.id, c.code, c.name
             FROM customers c
             WHERE {column} LIKE ?1 COLLATE NOCASE ESCAPE '\\'
             LIMIT {limit}"
        );

        let mut statement = self.connection.prepare(&sql)?;
        let mut hits = Vec::new();
        for variant in variants {
            let pattern = format!("{variant}%");
            let rows = statement.query_map([pattern.as_str()], |row| {
                let id: i64 = row.get(0)?;
                let code: String = row.get(1)?;
                let name: String = row.get(2)?;
                Ok(SearchHit {
                    kind: SearchKind::Customer,
                    id,
                    title: name,
                    subtitle: format!("Cari kodu: {code}"),
                })
            })?;
            hits.extend(rows.collect::<std::result::Result<Vec<_>, _>>()?);
        }
        Ok(hits)
    }

    fn search_products(&self, variants: &[String], limit: usize) -> Result<Vec<SearchHit>> {
        // Barkod kolonunda indeks yoktur; büyük veri setinde tam tablo taramasını
        // önlemek için arama yalnızca indeksli kod ve ad kolonlarında yapılır.
        let mut hits = self.search_products_by("p.code", variants, limit)?;
        hits.extend(self.search_products_by("p.name", variants, limit)?);
        Ok(merge_hits(hits, limit))
    }

    fn search_products_by(
        &self,
        column: &str,
        variants: &[String],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let sql = format!(
            "SELECT p.id, p.code, p.name, p.barcode
             FROM products p
             WHERE {column} LIKE ?1 COLLATE NOCASE ESCAPE '\\'
             LIMIT {limit}"
        );

        let mut statement = self.connection.prepare(&sql)?;
        let mut hits = Vec::new();
        for variant in variants {
            let pattern = format!("{variant}%");
            let rows = statement.query_map([pattern.as_str()], |row| {
                let id: i64 = row.get(0)?;
                let code: String = row.get(1)?;
                let name: String = row.get(2)?;
                let barcode: Option<String> = row.get(3)?;
                let subtitle = match barcode.filter(|value| !value.is_empty()) {
                    Some(barcode) => format!("Kod: {code} · Barkod: {barcode}"),
                    None => format!("Ürün kodu: {code}"),
                };
                Ok(SearchHit {
                    kind: SearchKind::Product,
                    id,
                    title: name,
                    subtitle,
                })
            })?;
            hits.extend(rows.collect::<std::result::Result<Vec<_>, _>>()?);
        }
        Ok(hits)
    }

    fn search_invoices(&self, variants: &[String], limit: usize) -> Result<Vec<SearchHit>> {
        let mut hits = self.search_invoices_by_number(variants, limit)?;
        hits.extend(self.search_invoices_by_customer(variants, limit)?);
        Ok(merge_hits(hits, limit))
    }

    fn search_invoices_by_number(
        &self,
        variants: &[String],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let sql = format!(
            "SELECT i.id, i.invoice_number, i.invoice_type, i.invoice_date,
                    i.grand_total, c.name
             FROM invoices i
             CROSS JOIN customers c
             WHERE c.id = i.customer_id
               AND i.invoice_number LIKE ?1 COLLATE NOCASE ESCAPE '\\'
             LIMIT {limit}"
        );

        self.collect_invoice_hits(&sql, variants)
    }

    fn search_invoices_by_customer(
        &self,
        variants: &[String],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        // Önce ada göre eşleşen carilerin küçük bir kümesini indeksten seç,
        // sonra yalnızca bu carilerin faturalarını `customer_id` indeksinden oku.
        // Böylece faturasız büyük cari tablosunda tüm cariler taranmaz; 100.000
        // faturalı veri setinde de her fatura için cari adı karşılaştırılmaz.
        let sql = format!(
            "SELECT i.id, i.invoice_number, i.invoice_type, i.invoice_date,
                    i.grand_total, c.name
             FROM (
                 SELECT id, name
                 FROM customers
                 WHERE name LIKE ?1 COLLATE NOCASE ESCAPE '\\'
                 LIMIT 1000
             ) c
             CROSS JOIN invoices i
             WHERE i.customer_id = c.id
             LIMIT {limit}"
        );

        self.collect_invoice_hits(&sql, variants)
    }

    fn collect_invoice_hits(&self, sql: &str, variants: &[String]) -> Result<Vec<SearchHit>> {
        let mut statement = self.connection.prepare(sql)?;
        let mut hits = Vec::new();
        for variant in variants {
            let pattern = format!("{variant}%");
            let rows = statement.query_map([pattern.as_str()], |row| {
                let id: i64 = row.get(0)?;
                let number: String = row.get(1)?;
                let invoice_type: String = row.get(2)?;
                let date: String = row.get(3)?;
                let total: i64 = row.get(4)?;
                let customer_name: String = row.get(5)?;
                let invoice_type = InvoiceType::from_db(&invoice_type).unwrap_or_default();
                let total = crate::money::Money::from_kurus(total).format_turkish();
                Ok(SearchHit {
                    kind: SearchKind::Invoice(invoice_type),
                    id,
                    title: number,
                    subtitle: format!(
                        "{} · {} · {} · {total}",
                        invoice_type.label(),
                        customer_name,
                        date
                    ),
                })
            })?;
            hits.extend(rows.collect::<std::result::Result<Vec<_>, _>>()?);
        }
        Ok(hits)
    }

    fn search_cash_accounts(&self, variants: &[String], limit: usize) -> Result<Vec<SearchHit>> {
        let mut hits = self.search_cash_accounts_by("a.name", variants, limit)?;
        hits.extend(self.search_cash_accounts_by("a.bank_name", variants, limit)?);
        Ok(merge_hits(hits, limit))
    }

    fn search_cash_accounts_by(
        &self,
        column: &str,
        variants: &[String],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let sql = format!(
            "SELECT a.id, a.name, a.account_type, a.bank_name
             FROM cash_accounts a
             WHERE {column} LIKE ?1 COLLATE NOCASE ESCAPE '\\'
             LIMIT {limit}"
        );

        let mut statement = self.connection.prepare(&sql)?;
        let mut hits = Vec::new();
        for variant in variants {
            let pattern = format!("{variant}%");
            let rows = statement.query_map([pattern.as_str()], |row| {
                let id: i64 = row.get(0)?;
                let name: String = row.get(1)?;
                let account_type: String = row.get(2)?;
                let bank_name: Option<String> = row.get(3)?;
                let subtitle = match bank_name.filter(|value| !value.is_empty()) {
                    Some(bank_name) => format!("{account_type} · {bank_name}"),
                    None => account_type,
                };
                Ok(SearchHit {
                    kind: SearchKind::CashAccount,
                    id,
                    title: name,
                    subtitle,
                })
            })?;
            hits.extend(rows.collect::<std::result::Result<Vec<_>, _>>()?);
        }
        Ok(hits)
    }
}

/// Aynı kaydı birden çok kolon sorgusundan gelen sonuçlardan tekilleştirir,
/// başlığa göre sıralar ve grup sınırını uygular.
fn merge_hits(hits: Vec<SearchHit>, limit: usize) -> Vec<SearchHit> {
    let mut seen = HashSet::new();
    let mut merged: Vec<SearchHit> = hits
        .into_iter()
        .filter(|hit| seen.insert((hit.kind, hit.id)))
        .collect();
    merged.sort_by(|left, right| {
        left.title
            .to_lowercase()
            .cmp(&right.title.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    merged.truncate(limit);
    merged
}

/// `LIKE` için `%` ve `_` karakterlerini kaçırır.
pub fn escape_like(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '%' | '_' | '\\' => {
                escaped.push('\\');
                escaped.push(character);
            }
            other => escaped.push(other),
        }
    }
    escaped
}

/// Normalize edilmiş arama metninden sınırlı sayıda Türkçe yazım varyantı
/// üretir. En fazla 128 varyant döner; böylece SQL sorgusu kontrollü kalır.
pub fn search_like_variants(normalized: &str) -> Vec<String> {
    let mut variants = vec![String::new()];

    for character in normalized.chars() {
        let alternatives = turkish_alternatives(character);
        let mut next = Vec::with_capacity(variants.len() * alternatives.len());
        for prefix in &variants {
            for alternative in &alternatives {
                let mut value = prefix.clone();
                value.push(*alternative);
                next.push(value);
                if next.len() >= 128 {
                    break;
                }
            }
            if next.len() >= 128 {
                break;
            }
        }
        variants = next;
    }

    variants
        .into_iter()
        .map(|variant| escape_like(&variant))
        .collect()
}

fn turkish_alternatives(character: char) -> Vec<char> {
    // SQLite'ın NOCASE eşlemesi yalnızca ASCII harflerde çalışır. Bu nedenle
    // Türkçe harflerin **hem küçük hem büyük** yazımı üretilir; aksi hâlde
    // "Ürün" kaydı "urun" aramasıyla bulunamaz.
    match character {
        'a' => vec!['a', 'â', 'Â'],
        'c' => vec!['c', 'ç', 'Ç'],
        'g' => vec!['g', 'ğ', 'Ğ'],
        'i' => vec!['i', 'ı', 'İ'],
        'o' => vec!['o', 'ö', 'Ö'],
        's' => vec!['s', 'ş', 'Ş'],
        'u' => vec!['u', 'ü', 'Ü', 'û', 'Û'],
        other => vec![other],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turkce_karakter_varyantlari_uretilir() {
        let variants = search_like_variants("ozturk");
        assert!(variants.iter().any(|value| value.contains('ö')));
        assert!(variants.iter().any(|value| value.contains('ü')));
        assert!(variants.len() <= 128);
    }

    #[test]
    fn buyuk_harfli_turkce_yazim_varyanti_uretilir() {
        let variants = search_like_variants("urun");
        assert!(variants.iter().any(|value| value == "Ürün"));
    }

    #[test]
    fn like_ozel_karakterleri_kacirilir() {
        assert_eq!(escape_like("50%_a\\b"), "50\\%\\_a\\\\b");
    }

    #[test]
    fn arama_grubu_etiketleri_bos_degildir() {
        for group in [
            SearchGroup::Customers,
            SearchGroup::Products,
            SearchGroup::Invoices,
            SearchGroup::CashAccounts,
        ] {
            assert!(!group.label().is_empty());
        }
    }
}
