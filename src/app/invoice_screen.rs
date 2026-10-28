// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Fatura ekranlarının durumu ve komutları.
//!
//! Arayüz katmanı faturalarla ilgili **hiçbir SQL çalıştırmaz**. Ekranın
//! gösterdiği tüm veri [`InvoiceState`] içinde tutulur ve tüm değişiklikler bu
//! modüldeki komut metotları (`open_invoice_new_form`, `submit_invoice_form`,
//! `confirm_invoice_save`, `submit_invoice_cancel`, ...) ile yapılır.
//!
//! ## Kaydetme akışı
//!
//! ```text
//! Form (InvoiceDraft) --validate--> ValidatedInvoice --(onay)--> [stok eksiye düşer mi?]
//!        ^                                   |                         |          |
//!        |  hata: FieldErrors (girdi korunur) |                  hayır  |          | evet: onay sorulur
//!        +-----------------------------------+                         v          v
//!                                                        fatura kesinleşir   kullanıcı onaylarsa kesinleşir
//! ```
//!
//! Fatura oluşturulduktan sonra **düzenlenemez**: düzeltme, iptal edip yeni bir
//! fatura kesmekle yapılır. İptal işlemi ters yönlü stok hareketleri üretir.

use crate::customer::{Customer, CustomerFilter};
use crate::error::AppError;
use crate::invoice::{
    Field, FieldErrors, Invoice, InvoiceDraft, InvoiceFilter, InvoiceLine, InvoiceLineDraft,
    InvoiceStatus, InvoiceType, ValidatedInvoice, validate_cancel_reason, year_of,
};
use crate::product::{Product, ProductFilter};
use crate::quantity::Quantity;

use super::{FocusTarget, OnMuhasebeApp};

/// Fatura bölümündeki alt ekranlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InvoiceScreen {
    /// Fatura listesi.
    #[default]
    List,
    /// Yeni fatura formu.
    Form,
    /// Fatura ayrıntısı.
    Detail,
}

/// Fatura bölümünün tüm durumu (liste, form ve ayrıntı).
#[derive(Debug, Default)]
pub struct InvoiceState {
    /// Aktif alt ekran.
    pub screen: InvoiceScreen,
    /// Bölümün türü (Alış/Satış); yeni formun varsayılanı ve listede gösterilen.
    pub invoice_type: InvoiceType,
    /// Ayrıntısı gösterilen faturanın kimliği.
    pub detail_id: Option<i64>,
    /// Yeni faturanın ham form verisi.
    pub draft: InvoiceDraft,
    /// Form açıldığındaki başlangıç değerleri.
    pub original: InvoiceDraft,
    /// Form hataları (satır hataları dâhil).
    pub errors: FieldErrors,
    /// Liste ölçütleri.
    pub filter: InvoiceFilter,
    /// Listelenen faturalar.
    pub list: Vec<Invoice>,
    /// Liste okunurken oluşan hata.
    pub list_error: Option<String>,
    /// Ayrıntısı okunan fatura.
    pub selected: Option<Invoice>,
    /// Ayrıntısı okunan faturanın satırları.
    pub selected_lines: Vec<InvoiceLine>,
    /// Seçim listeleri için okunmuş cariler.
    pub customers: Vec<Customer>,
    /// Satır seçimi için okunmuş ürünler.
    pub products: Vec<Product>,
    /// Kaydetme onayı bekleyen doğrulanmış fatura.
    pub pending: Option<ValidatedInvoice>,
    /// İptal nedeni girdisi.
    pub cancel_reason: String,
    /// İptal nedeni hatası.
    pub cancel_error: Option<String>,
    /// İptal onay penceresi açık mı?
    pub cancelling: bool,
    /// Bilgilendirme mesajı.
    pub notice: Option<String>,
    /// Genel hata mesajı.
    pub general_error: Option<String>,
}

impl OnMuhasebeApp {
    /// Fatura bölümünün durumunu okur.
    pub fn invoice_state(&self) -> &InvoiceState {
        &self.invoice
    }

    /// Fatura bölümünün durumunu değiştirilebilir olarak verir (form alanları).
    pub fn invoice_state_mut(&mut self) -> &mut InvoiceState {
        &mut self.invoice
    }

    /// Fatura formunda kaydedilmemiş değişiklik var mı?
    pub fn invoice_form_dirty(&self) -> bool {
        self.invoice.screen == InvoiceScreen::Form && self.invoice.draft != self.invoice.original
    }

    /// Bölüme (Alış/Satış) geçildiğinde durumu hazırlar ve listeyi yeniler.
    pub fn set_invoice_section(&mut self, invoice_type: InvoiceType) {
        self.invoice.invoice_type = invoice_type;
        self.invoice.screen = InvoiceScreen::List;
        self.invoice.errors = FieldErrors::default();
        self.invoice.general_error = None;
        self.invoice.pending = None;
        self.invoice.cancelling = false;
        self.invoice.cancel_error = None;
        self.invoice.notice = None;
        self.reload_invoice_customers();
        self.refresh_invoice_list();
    }

    /// Kayıt listesini geçerli ölçütlere göre veritabanından yeniden okur.
    pub fn refresh_invoice_list(&mut self) {
        self.invoice.list_error = None;

        let invoice_type = self.invoice.invoice_type;
        let filter = self.invoice.filter.clone();

        let Some(database) = &self.database else {
            self.invoice.list = Vec::new();
            self.invoice.list_error =
                Some("Veritabanı bağlantısı kurulamadığı için faturalar yüklenemedi.".to_owned());
            return;
        };

        match database.invoice_list(invoice_type, &filter) {
            Ok(invoices) => self.invoice.list = invoices,
            Err(error) => {
                self.invoice.list = Vec::new();
                self.invoice.list_error = Some(error.to_string());
            }
        }
    }

    /// Arama ve filtreleri temizler ve listeyi yeniler.
    pub fn reset_invoice_filter(&mut self) {
        self.invoice.filter = InvoiceFilter::default();
        self.refresh_invoice_list();
    }

    /// Listeye döner ve listeyi yeniler.
    pub fn open_invoice_list(&mut self) {
        self.invoice.screen = InvoiceScreen::List;
        self.invoice.errors = FieldErrors::default();
        self.invoice.general_error = None;
        self.invoice.pending = None;
        self.invoice.cancelling = false;
        self.request_scroll_reset();
        self.reload_invoice_customers();
        self.refresh_invoice_list();
    }

    /// Yeni fatura formunu açar; numara önerilir.
    pub fn open_invoice_new_form(&mut self) {
        self.reload_invoice_customers();
        self.reload_invoice_products();

        self.invoice.draft = InvoiceDraft::new(self.invoice.invoice_type);
        self.invoice.errors = FieldErrors::default();
        self.invoice.general_error = None;
        self.invoice.pending = None;
        self.invoice.cancel_reason.clear();
        self.invoice.cancel_error = None;
        self.invoice.cancelling = false;
        self.invoice.screen = InvoiceScreen::Form;
        self.request_scroll_reset();
        self.suggest_invoice_number();
        self.invoice.original = self.invoice.draft.clone();
        self.request_focus(FocusTarget::InvoiceNumber);
    }

    /// Formu kaydetmeden kapatır ve listeye döner.
    pub fn cancel_invoice_form(&mut self) {
        self.invoice.errors = FieldErrors::default();
        self.invoice.general_error = None;
        self.invoice.pending = None;
        self.invoice.screen = InvoiceScreen::List;
        self.request_scroll_reset();
        self.refresh_invoice_list();
    }

    /// Fatura numarası alanı için kullanılmamış bir numara önerir.
    pub fn suggest_invoice_number(&mut self) {
        let invoice_type = self.invoice.draft.invoice_type;
        let year = year_of(&self.invoice.draft.invoice_date);

        let number = match &self.database {
            Some(database) => match database.invoice_next_number(invoice_type, year) {
                Ok(number) => number,
                Err(error) => {
                    self.invoice.general_error = Some(error.to_string());
                    return;
                }
            },
            None => return,
        };

        self.invoice.draft.number = number;
        self.invoice.errors.clear(Field::Number);
    }

    /// Yeni formun türünü değiştirir (numara serisi buna göre yeniden önerilir).
    pub fn set_invoice_draft_type(&mut self, invoice_type: InvoiceType) {
        if self.invoice.draft.invoice_type == invoice_type {
            return;
        }
        self.invoice.draft.invoice_type = invoice_type;
        self.suggest_invoice_number();
    }

    /// Forma boş bir satır ekler.
    pub fn add_invoice_line(&mut self) {
        self.invoice.draft.lines.push(InvoiceLineDraft::default());
        self.invoice.errors.clear(Field::Lines);
    }

    /// Formdaki bir satırı kaldırır (satır hataları indeks kayması nedeniyle temizlenir).
    pub fn remove_invoice_line(&mut self, index: usize) {
        if index >= self.invoice.draft.lines.len() {
            return;
        }
        self.invoice.draft.lines.remove(index);
        self.invoice.errors.clear_lines();
    }

    /// Bir satıra ürün atar ve ürün kartından varsayılan değerleri doldurur.
    ///
    /// Varsayılanlar: ürün kodu/adı, birim, birim fiyat (alışta alış fiyatı,
    /// satışta satış fiyatı) ve KDV oranı. Fiyat ve KDV sonradan düzenlenebilir.
    pub fn apply_invoice_line_product(&mut self, index: usize, product_id: i64) {
        let invoice_type = self.invoice.draft.invoice_type;
        let product = self
            .invoice
            .products
            .iter()
            .find(|product| product.id == product_id)
            .cloned();

        let Some(line) = self.invoice.draft.lines.get_mut(index) else {
            return;
        };

        if line.product_id == Some(product_id) {
            return;
        }

        line.product_id = Some(product_id);
        if let Some(product) = product {
            line.product_code = product.code.clone();
            line.product_name = product.name.clone();
            line.unit = product.unit;
            let price = match invoice_type {
                InvoiceType::Purchase => product.purchase_price,
                InvoiceType::Sales => product.sale_price,
            };
            line.unit_price = price.format_turkish();
            line.vat_rate = product.vat_rate.to_string();
        }

        self.invoice.errors.clear_line_all(index);
    }

    /// Formdaki veriyi doğrular; başarılıysa kaydetme onayı için bekletir.
    pub fn submit_invoice_form(&mut self) {
        match self.invoice.draft.validate() {
            Ok(validated) => {
                self.invoice.errors = FieldErrors::default();
                self.invoice.general_error = None;
                self.invoice.pending = Some(validated);
            }
            Err(errors) => {
                let focus = if errors.get(Field::Customer).is_some() {
                    FocusTarget::InvoiceCustomer
                } else {
                    FocusTarget::InvoiceNumber
                };
                self.invoice.pending = None;
                self.invoice.general_error = None;
                self.invoice.errors = errors;
                self.request_focus(focus);
            }
        }
    }

    /// Kaydetme onayını iptal eder (form açık kalır, girdi korunur).
    pub fn discard_invoice_confirmation(&mut self) {
        self.invoice.pending = None;
    }

    /// Onaylanan faturayı kesinleştirir; stok ve cari bakiyesi güncellenir.
    pub fn confirm_invoice_save(&mut self) {
        let Some(validated) = self.invoice.pending.clone() else {
            return;
        };

        let result = match &self.database {
            Some(database) => database.invoice_insert(&validated),
            None => {
                self.invoice.general_error =
                    Some("Veritabanı bağlantısı yok; fatura kaydedilemedi.".to_owned());
                return;
            }
        };

        match result {
            Ok(id) => {
                self.invoice.pending = None;
                self.invoice.notice = Some(
                    "Fatura kesinleşti, düzenlenemez. Düzeltmek için iptal edip yeniden \
                     oluşturun."
                        .to_owned(),
                );
                self.refresh_invoice_list();
                self.refresh_customer_list();
                self.refresh_product_list();
                self.open_invoice_detail(id);
            }
            Err(AppError::InvoiceNumberTaken(number)) => {
                self.invoice.pending = None;
                self.invoice.screen = InvoiceScreen::Form;
                self.invoice.errors.push(
                    Field::Number,
                    format!("'{number}' numarası zaten kullanılıyor."),
                );
            }
            Err(error) => {
                self.invoice.pending = None;
                self.invoice.screen = InvoiceScreen::Form;
                self.invoice.general_error = Some(error.to_string());
            }
        }
    }

    /// Faturanın ayrıntısını açar.
    pub fn open_invoice_detail(&mut self, id: i64) {
        let invoice = match &self.database {
            Some(database) => match database.invoice_get(id) {
                Ok(Some(invoice)) => invoice,
                Ok(None) => {
                    self.invoice.general_error =
                        Some(format!("Fatura bulunamadı (kayıt no: {id})."));
                    self.open_invoice_list();
                    return;
                }
                Err(error) => {
                    self.invoice.general_error = Some(error.to_string());
                    self.open_invoice_list();
                    return;
                }
            },
            None => {
                self.invoice.general_error =
                    Some("Veritabanı bağlantısı yok; fatura okunamadı.".to_owned());
                self.open_invoice_list();
                return;
            }
        };

        let lines = match &self.database {
            Some(database) => database.invoice_lines(id).unwrap_or_default(),
            None => Vec::new(),
        };

        self.invoice.selected = Some(invoice);
        self.invoice.selected_lines = lines;
        self.invoice.detail_id = Some(id);
        self.invoice.screen = InvoiceScreen::Detail;
        self.invoice.errors = FieldErrors::default();
        self.invoice.pending = None;
        self.invoice.cancelling = false;
        self.invoice.cancel_reason.clear();
        self.invoice.cancel_error = None;
        self.invoice.general_error = None;
        self.request_scroll_reset();
    }

    /// Listeden iptal akışını başlatır (ayrıntıyı açar ve iptal penceresini gösterir).
    pub fn open_invoice_cancel(&mut self, id: i64) {
        self.open_invoice_detail(id);
        if self.invoice.selected.as_ref().map(|invoice| invoice.status)
            == Some(InvoiceStatus::Confirmed)
        {
            self.start_invoice_cancel();
        }
    }

    /// İptal onay penceresini açar.
    pub fn start_invoice_cancel(&mut self) {
        self.invoice.cancelling = true;
        self.invoice.cancel_reason.clear();
        self.invoice.cancel_error = None;
    }

    /// İptal onay penceresini kapatır (hiçbir şey değişmez).
    pub fn cancel_invoice_cancel(&mut self) {
        self.invoice.cancelling = false;
        self.invoice.cancel_reason.clear();
        self.invoice.cancel_error = None;
    }

    /// İptal nedenini doğrular ve faturayı iptal eder.
    pub fn submit_invoice_cancel(&mut self) {
        let reason = match validate_cancel_reason(&self.invoice.cancel_reason) {
            Ok(reason) => reason,
            Err(message) => {
                self.invoice.cancel_error = Some(message);
                return;
            }
        };

        let Some(id) = self.invoice.detail_id else {
            self.invoice.general_error = Some("Fatura bilgisi bulunamadı.".to_owned());
            return;
        };

        let result = match &self.database {
            Some(database) => database.invoice_cancel(id, &reason),
            None => {
                self.invoice.general_error =
                    Some("Veritabanı bağlantısı yok; fatura iptal edilemedi.".to_owned());
                return;
            }
        };

        match result {
            Ok(()) => {
                self.invoice.cancelling = false;
                self.invoice.cancel_reason.clear();
                self.invoice.cancel_error = None;
                self.invoice.notice =
                    Some("Fatura iptal edildi. Stok ve cari bakiyesi geri alındı.".to_owned());
                self.refresh_invoice_list();
                self.refresh_customer_list();
                self.refresh_product_list();
                self.open_invoice_detail(id);
            }
            Err(AppError::InvoiceAlreadyCancelled(_)) => {
                self.invoice.cancelling = false;
                self.invoice.general_error = Some("Bu fatura zaten iptal edilmiş.".to_owned());
                self.open_invoice_detail(id);
            }
            Err(error) => {
                self.invoice.cancel_error = Some(error.to_string());
            }
        }
    }

    /// Bilgilendirme mesajını kapatır.
    pub fn dismiss_invoice_notice(&mut self) {
        self.invoice.notice = None;
    }

    /// Form seçimi için cari listesini (uygun türdekiler önce) hazırlar.
    pub fn invoice_customer_options(&self, invoice_type: InvoiceType) -> Vec<(i64, String)> {
        let mut suitable = Vec::new();
        let mut other = Vec::new();

        for customer in &self.invoice.customers {
            let label = if invoice_type.is_customer_kind_suitable(customer.kind) {
                format!("{} — {}", customer.code, customer.name)
            } else {
                format!("{} — {} · tür uygun değil", customer.code, customer.name)
            };

            if invoice_type.is_customer_kind_suitable(customer.kind) {
                suitable.push((customer.id, label));
            } else {
                other.push((customer.id, label));
            }
        }

        suitable.extend(other);
        suitable
    }

    /// Satır seçimi için ürün listesini hazırlar.
    pub fn invoice_product_options(&self) -> Vec<(i64, String)> {
        self.invoice
            .products
            .iter()
            .map(|product| (product.id, format!("{} — {}", product.code, product.name)))
            .collect()
    }

    /// Filtre için tüm carileri (tür ayrımı yapmadan) hazırlar.
    pub fn invoice_all_customer_options(&self) -> Vec<(i64, String)> {
        self.invoice
            .customers
            .iter()
            .map(|customer| {
                (
                    customer.id,
                    format!("{} — {}", customer.code, customer.name),
                )
            })
            .collect()
    }

    /// Cari etiketini kimliğine göre verir.
    pub fn invoice_customer_label(&self, customer_id: i64) -> Option<String> {
        self.invoice
            .customers
            .iter()
            .find(|customer| customer.id == customer_id)
            .map(|customer| format!("{} — {}", customer.code, customer.name))
    }

    /// Satır seçimi için ürün etiketini kimliğine göre verir.
    pub fn invoice_product_label(&self, product_id: i64) -> Option<String> {
        self.invoice
            .products
            .iter()
            .find(|product| product.id == product_id)
            .map(|product| format!("{} — {}", product.code, product.name))
    }

    /// Formda seçili cariyi döndürür (varsa).
    pub fn invoice_selected_customer(&self) -> Option<&Customer> {
        let id = self.invoice.draft.customer_id?;
        self.invoice
            .customers
            .iter()
            .find(|customer| customer.id == id)
    }

    /// Bir cariye bağlı faturaları okur (cari ayrıntısı için).
    pub fn customer_invoices(&self, customer_id: i64) -> Vec<Invoice> {
        match &self.database {
            Some(database) => database
                .invoice_list_for_customer(customer_id)
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// Onay öncesi hangi ürünlerin stoğunun eksiye düşeceğini bulur.
    ///
    /// Dönen değer her etkilenen ürün için `(etiket, mevcut stok, yeni stok)`
    /// üçlüsüdür. Satış faturasında stok yetersizse kullanıcıdan onay istenir.
    pub fn invoice_stock_shortages(
        &self,
        invoice: &ValidatedInvoice,
    ) -> Vec<(String, Quantity, Quantity)> {
        let sign = invoice.invoice_type.stock_sign();
        let mut shortages = Vec::new();

        for line in &invoice.lines {
            let Some(product) = self
                .invoice
                .products
                .iter()
                .find(|product| product.id == line.product_id)
            else {
                continue;
            };

            let Some(delta) = line.quantity.milli().checked_mul(sign) else {
                continue;
            };
            let Ok(new_stock) = product
                .current_stock
                .checked_add(Quantity::from_milli(delta))
            else {
                continue;
            };

            if new_stock.is_negative() {
                shortages.push((
                    format!("{} — {}", product.code, product.name),
                    product.current_stock,
                    new_stock,
                ));
            }
        }

        shortages
    }

    /// Seçim listeleri için carileri yeniden okur.
    fn reload_invoice_customers(&mut self) {
        let Some(database) = &self.database else {
            self.invoice.customers = Vec::new();
            return;
        };
        match database.customer_list(&CustomerFilter::default()) {
            Ok(customers) => self.invoice.customers = customers,
            Err(error) => self.invoice.general_error = Some(error.to_string()),
        }
    }

    /// Satır seçimi için ürünleri yeniden okur.
    fn reload_invoice_products(&mut self) {
        let Some(database) = &self.database else {
            self.invoice.products = Vec::new();
            return;
        };
        match database.product_list(&ProductFilter::default()) {
            Ok(products) => self.invoice.products = products,
            Err(error) => self.invoice.general_error = Some(error.to_string()),
        }
    }
}
