// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Global arama paletinin uygulama durumu ve komutları.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use crate::customer::normalize_for_search;
use crate::db::{Database, SearchGroup, SearchHit, SearchKind};
use crate::invoice::InvoiceType;

use super::{FocusTarget, OnMuhasebeApp, View};

type SearchResult = std::result::Result<Vec<SearchHit>, String>;

/// Arama paletinin gecikmeli sorgu durumu.
#[derive(Default)]
pub struct SearchPaletteState {
    /// Palet açık mı?
    pub open: bool,
    /// Arama metni.
    pub query: String,
    /// Son sorgunun sonuçları.
    pub results: Vec<SearchHit>,
    /// Klavyeyle seçili sonuç indeksi.
    pub selected: usize,
    /// Arama hatası.
    pub error: Option<String>,
    /// Metin değiştiğinden beri geçen sürenin başlangıcı.
    pub changed_at: Option<Instant>,
    /// En son çalıştırılan sorgu.
    pub last_query: String,
    /// Arka plan araması sürüyor mu?
    pub searching: bool,
    /// Arka plan aramasının sonucunu taşıyan kanal.
    pending: Option<Receiver<(u64, SearchResult)>>,
    /// Eski bir arka plan sonucunu yok saymak için istek numarası.
    request_id: u64,
}

impl OnMuhasebeApp {
    /// Arama paleti durumunu okur.
    pub fn search_state(&self) -> &SearchPaletteState {
        &self.search
    }

    /// Arama paleti durumunu değiştirilebilir olarak verir.
    pub fn search_state_mut(&mut self) -> &mut SearchPaletteState {
        &mut self.search
    }

    /// Arama paletini açar ve arama alanına odaklanır.
    pub fn open_search_palette(&mut self) {
        self.search.open = true;
        self.search.query.clear();
        self.search.results.clear();
        self.search.selected = 0;
        self.search.error = None;
        self.search.changed_at = None;
        self.search.last_query.clear();
        self.search.searching = false;
        self.search.pending = None;
        self.search.request_id = self.search.request_id.wrapping_add(1);
        self.focus_target = Some(FocusTarget::SearchQuery);
    }

    /// Arama paletini kapatır.
    pub fn close_search_palette(&mut self) {
        self.search.open = false;
        self.search.query.clear();
        self.search.results.clear();
        self.search.selected = 0;
        self.search.error = None;
        self.search.changed_at = None;
        self.search.last_query.clear();
        self.search.searching = false;
        self.search.pending = None;
        self.search.request_id = self.search.request_id.wrapping_add(1);
    }

    /// Arama metni değiştiğinde 150 ms'lik gecikmeyi başlatır.
    pub fn search_query_changed(&mut self) {
        self.search.error = None;
        let normalized = normalize_for_search(self.search.query.trim());
        if normalized.chars().count() < 2 {
            self.search.results.clear();
            self.search.selected = 0;
            self.search.changed_at = None;
            self.search.last_query.clear();
            return;
        }
        self.search.changed_at = Some(Instant::now());
    }

    /// Gecikme dolduysa aramayı arka planda başlatır; çağrı her karede yapılabilir.
    pub fn tick_search(&mut self) {
        if !self.search.open {
            return;
        }
        self.poll_search_result();
        if let Some(changed_at) = self.search.changed_at
            && changed_at.elapsed() >= Duration::from_millis(150)
        {
            self.start_search_async();
        }
    }

    /// Aramayı beklemeden hemen çalıştırır (testler ve Enter tuşu).
    pub fn run_search_now(&mut self) {
        self.search.changed_at = None;
        self.search.searching = false;
        self.search.pending = None;
        self.search.request_id = self.search.request_id.wrapping_add(1);
        let normalized = normalize_for_search(self.search.query.trim());
        if normalized.chars().count() < 2 {
            self.search.results.clear();
            self.search.selected = 0;
            self.search.last_query.clear();
            return;
        }

        let query = self.search.query.trim().to_owned();
        self.search.last_query = query.clone();
        let result = match self.database.as_ref() {
            Some(database) => database.global_search(&query),
            None => Err(crate::error::AppError::Maintenance(
                "Veritabanı açık değil.".to_owned(),
            )),
        };

        match result {
            Ok(results) => {
                self.search.results = results;
                self.search.selected = 0;
                self.search.error = None;
            }
            Err(error) => {
                self.search.results.clear();
                self.search.selected = 0;
                self.search.error = Some(error.to_string());
            }
        }
    }

    /// Ağır aramayı ayrı bir bağlantıda başlatır; arayüz kilitlenmez.
    fn start_search_async(&mut self) {
        self.search.changed_at = None;
        let normalized = normalize_for_search(self.search.query.trim());
        if normalized.chars().count() < 2 {
            self.search.results.clear();
            self.search.selected = 0;
            self.search.last_query.clear();
            self.search.searching = false;
            self.search.pending = None;
            return;
        }

        let Some(path) = self
            .database
            .as_ref()
            .and_then(Database::path)
            .map(Path::to_path_buf)
        else {
            self.search.error = Some("Veritabanı açık değil.".to_owned());
            self.search.results.clear();
            self.search.searching = false;
            return;
        };

        let query = self.search.query.trim().to_owned();
        let request_id = self.search.request_id.wrapping_add(1);
        self.search.request_id = request_id;
        self.search.last_query = query.clone();
        self.search.error = None;
        self.search.searching = true;

        let (sender, receiver) = mpsc::channel();
        self.search.pending = Some(receiver);
        thread::spawn(move || {
            let result = Database::open(&path)
                .map_err(|error| error.to_string())
                .and_then(|database| {
                    database
                        .global_search(&query)
                        .map_err(|error| error.to_string())
                });
            let _ = sender.send((request_id, result));
        });
    }

    /// Arka plan araması tamamlandıysa sonucu uygular.
    fn poll_search_result(&mut self) {
        let message = {
            let Some(receiver) = self.search.pending.as_ref() else {
                return;
            };
            receiver.try_recv()
        };

        match message {
            Ok((request_id, result)) => {
                if request_id != self.search.request_id {
                    return;
                }
                self.search.pending = None;
                self.search.searching = false;
                match result {
                    Ok(results) => {
                        self.search.results = results;
                        self.search.selected = 0;
                        self.search.error = None;
                    }
                    Err(error) => {
                        self.search.results.clear();
                        self.search.selected = 0;
                        self.search.error = Some(error);
                    }
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.search.pending = None;
                self.search.searching = false;
                self.search.error = Some("Arama tamamlanamadı.".to_owned());
            }
        }
    }

    /// Seçili sonucu `delta` kadar kaydırır (döngüsel).
    pub fn search_move(&mut self, delta: isize) {
        let count = self.search.results.len();
        if count == 0 {
            self.search.selected = 0;
            return;
        }
        let current = self.search.selected.min(count - 1) as isize;
        let next = (current + delta).rem_euclid(count as isize) as usize;
        self.search.selected = next;
    }

    /// Seçili sonucu döndürür.
    pub fn search_selected_hit(&self) -> Option<&SearchHit> {
        self.search.results.get(self.search.selected)
    }

    /// Seçili sonucu açar. Sonuç yoksa `false` döner.
    pub fn search_activate_selected(&mut self) -> bool {
        let Some(hit) = self.search_selected_hit().cloned() else {
            return false;
        };
        self.navigate_to_search_hit(&hit);
        true
    }

    /// Arama sonucuna göre ilgili ekranı ve kaydı açar.
    pub fn navigate_to_search_hit(&mut self, hit: &SearchHit) {
        self.close_search_palette();
        match hit.kind {
            SearchKind::Customer => {
                self.set_view(View::Customers);
                self.open_customer_detail(hit.id);
            }
            SearchKind::Product => {
                self.set_view(View::Inventory);
                self.open_product_detail(hit.id);
            }
            SearchKind::Invoice(invoice_type) => {
                let view = match invoice_type {
                    InvoiceType::Purchase => View::PurchaseInvoices,
                    InvoiceType::Sales => View::SalesInvoices,
                };
                self.set_view(view);
                self.set_invoice_section(invoice_type);
                self.open_invoice_detail(hit.id);
            }
            SearchKind::CashAccount => {
                self.show_cash_account_highlight(hit.id);
            }
        }
    }

    /// Sonuçları grup başlıklarına göre döndürür.
    pub fn search_grouped_results(&self) -> Vec<(SearchGroup, Vec<SearchHit>)> {
        let mut grouped = Vec::new();
        for group in [
            SearchGroup::Customers,
            SearchGroup::Products,
            SearchGroup::Invoices,
            SearchGroup::CashAccounts,
        ] {
            let hits: Vec<SearchHit> = self
                .search
                .results
                .iter()
                .filter(|hit| hit.kind.group() == group)
                .cloned()
                .collect();
            if !hits.is_empty() {
                grouped.push((group, hits));
            }
        }
        grouped
    }
}
