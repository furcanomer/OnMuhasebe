// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ürün ve stok ekranının durumu ve komutları.
//!
//! Arayüz katmanı ürünlerle ilgili **hiçbir SQL çalıştırmaz**. Ekranın
//! gösterdiği tüm veri [`ProductState`] içinde tutulur ve tüm değişiklikler
//! bu modüldeki komut metotları (`open_product_new_form`,
//! `submit_product_form`, `submit_movement_form`, ...) ile yapılır.
//!
//! ## Stok hareketi akışı
//!
//! ```text
//! Ayrıntı --(Yeni Hareket)--> Hareket formu --(Kaydet)--> [stok eksiye düşer mi?]
//!                                                              |          |
//!                                                     hayır    |          | evet: onay sorulur
//!                                                              v          v
//!                                                    hareket kaydedilir   kullanıcı onaylarsa kaydedilir
//! ```
//!
//! Kaydetme başarısız olursa (doğrulama veya veritabanı hatası) kullanıcının
//! girdiği bilgiler **silinmez**; form ekranında kalır. Hareketler doğrudan
//! düzenlenemez veya silinemez; düzeltme, ters yönlü bir `duzeltme`
//! hareketiyle yapılır.

use crate::error::AppError;
use crate::invoice::Invoice;
use crate::money::Money;
use crate::product::{
    Field, FieldErrors, MovementDraft, Product, ProductDraft, ProductFilter, StockMovement,
    ValidatedMovement, ValidatedProduct,
};
use crate::quantity::Quantity;

use super::{FocusTarget, OnMuhasebeApp};

/// Ürün ve stok bölümünde gösterilen alt ekranlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProductScreen {
    /// Kayıt listesi (arama ve filtreleme ile).
    #[default]
    List,
    /// Yeni kayıt / düzenleme formu.
    Form,
    /// Ürün ayrıntısı ve stok hareketleri.
    Detail,
    /// Yeni stok hareketi formu.
    MovementForm,
}

/// Ürün ve stok bölümünün çalışma zamanı durumu.
#[derive(Debug, Default)]
pub struct ProductState {
    /// Gösterilen alt ekran.
    pub screen: ProductScreen,
    /// Ayrıntısı gösterilen kaydın kimliği.
    pub detail_id: Option<i64>,
    /// Düzenlenen kaydın kimliği (`None` = yeni kayıt).
    pub form_id: Option<i64>,
    /// Form iptal edildiğinde dönülecek ekran.
    pub form_origin: ProductScreen,
    /// Formdaki ham kullanıcı girdisi.
    pub draft: ProductDraft,
    /// Form açıldığındaki başlangıç değerleri (değişiklik algılama).
    pub form_original: ProductDraft,
    /// Alan bazlı doğrulama hataları.
    pub errors: FieldErrors,
    /// Liste ölçütleri (arama, durum, düşük stok).
    pub filter: ProductFilter,
    /// Ölçütlere uyan kayıtlar.
    pub list: Vec<Product>,
    /// Liste yüklenirken oluşan hata.
    pub list_error: Option<String>,
    /// Ayrıntı ekranında gösterilen kayıt.
    pub selected: Option<Product>,
    /// Ayrıntı ekranında gösterilen stok hareketleri.
    pub movements: Vec<StockMovement>,
    /// Hareket formundaki ham kullanıcı girdisi.
    pub movement_draft: MovementDraft,
    /// Hareket formu açıldığındaki başlangıç değerleri.
    pub movement_original: MovementDraft,
    /// Hareket formunun alan bazlı hataları.
    pub movement_errors: FieldErrors,
    /// Stok eksiye düşecekse onay bekleyen tahmini yeni stok.
    pub movement_confirmation: Option<Quantity>,
    /// Son başarılı işlemin bilgilendirmesi.
    pub notice: Option<String>,
    /// Son başarısız işlemin hata mesajı.
    pub general_error: Option<String>,
}

/// Genel Bakış ekranının gerçek verilerden türettiği özet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OverviewMetrics {
    /// Aktif ürün sayısı.
    pub active_products: usize,
    /// Düşük stoklu ürün sayısı.
    pub low_stock_products: usize,
    /// Toplam cari hesap sayısı.
    pub customers: usize,
    /// Son stok hareketleri.
    pub recent_movements: Vec<StockMovement>,
    /// Kesinleşmiş satış faturalarının toplamı (KDV dâhil).
    pub sales_total: Money,
    /// Kesinleşmiş alış faturalarının toplamı (KDV dâhil).
    pub purchase_total: Money,
    /// Son faturalar.
    pub recent_invoices: Vec<Invoice>,
}

/// Kaydetme sırasında oluşabilecek hatalar.
///
/// Kod ve barkod çakışmaları alan hatası olarak gösterilir (form açık kalır),
/// diğer tüm sorunlar genel hata mesajı olarak gösterilir.
enum SaveError {
    /// Ürün kodu başka bir kayıtta kullanılıyor.
    CodeTaken(String),
    /// Barkod başka bir kayıtta kullanılıyor.
    BarcodeTaken(String),
    /// Diğer hatalar (veritabanı, bağlantı yok, ...).
    General(String),
}

impl OnMuhasebeApp {
    /// Ürün ve stok durumunu okur.
    pub fn product_state(&self) -> &ProductState {
        &self.product
    }

    /// Ürün ve stok durumunu değiştirilebilir olarak verir (form alanları).
    pub fn product_state_mut(&mut self) -> &mut ProductState {
        &mut self.product
    }

    /// Ürün formunda kaydedilmemiş değişiklik var mı?
    pub fn product_form_dirty(&self) -> bool {
        self.product.screen == ProductScreen::Form
            && self.product.draft != self.product.form_original
    }

    /// Stok hareketi formunda kaydedilmemiş değişiklik var mı?
    pub fn movement_form_dirty(&self) -> bool {
        self.product.screen == ProductScreen::MovementForm
            && self.product.movement_draft != self.product.movement_original
    }

    /// Kayıt listesini geçerli ölçütlere göre veritabanından yeniden okur.
    pub fn refresh_product_list(&mut self) {
        self.product.list_error = None;

        let Some(database) = &self.database else {
            self.product.list = Vec::new();
            self.product.list_error =
                Some("Veritabanı bağlantısı kurulamadığı için ürünler yüklenemedi.".to_owned());
            return;
        };

        match database.product_list(&self.product.filter) {
            Ok(products) => self.product.list = products,
            Err(error) => {
                self.product.list = Vec::new();
                self.product.list_error = Some(error.to_string());
            }
        }
    }

    /// Arama ve filtreleri temizler ve listeyi yeniler.
    pub fn reset_product_filter(&mut self) {
        self.product.filter = ProductFilter::default();
        self.refresh_product_list();
    }

    /// Listeye döner ve listeyi yeniler.
    pub fn open_product_list(&mut self) {
        self.product.screen = ProductScreen::List;
        self.product.errors = FieldErrors::default();
        self.product.general_error = None;
        self.request_scroll_reset();
        self.refresh_product_list();
    }

    /// Yalnızca düşük stoklu ürünleri gösterecek şekilde listeyi filtreler.
    ///
    /// Genel Bakış ekranındaki "düşük stok" bağlantısı bu komutu kullanır.
    pub fn show_low_stock_products(&mut self) {
        self.product.filter = ProductFilter {
            low_stock_only: true,
            ..ProductFilter::default()
        };
        self.product.screen = ProductScreen::List;
        self.product.errors = FieldErrors::default();
        self.product.general_error = None;
        self.set_view(super::View::Inventory);
        self.request_scroll_reset();
        self.refresh_product_list();
    }

    /// Yeni kayıt formunu açar; ürün kodu önerilir.
    pub fn open_product_new_form(&mut self) {
        self.product.draft = ProductDraft::default();
        self.product.errors = FieldErrors::default();
        self.product.general_error = None;
        self.product.form_id = None;
        self.product.form_origin = self.product.screen;
        self.product.screen = ProductScreen::Form;
        self.request_scroll_reset();
        self.suggest_product_code();
        self.product.form_original = self.product.draft.clone();
        self.request_focus(FocusTarget::ProductCode);
    }

    /// Var olan kaydı düzenlemek üzere formu açar.
    pub fn open_product_edit_form(&mut self, id: i64) {
        match self.load_product(id) {
            Ok(product) => {
                self.product.draft = ProductDraft::from_product(&product);
                self.product.errors = FieldErrors::default();
                self.product.general_error = None;
                self.product.form_id = Some(product.id);
                self.product.form_origin = self.product.screen;
                self.product.screen = ProductScreen::Form;
                self.product.form_original = self.product.draft.clone();
                self.request_scroll_reset();
                self.request_focus(FocusTarget::ProductCode);
            }
            Err(message) => {
                self.product.general_error = Some(message);
                self.open_product_list();
            }
        }
    }

    /// Formu kaydetmeden kapatır ve geldiği ekrana döner.
    pub fn cancel_product_form(&mut self) {
        let origin = self.product.form_origin;
        self.product.errors = FieldErrors::default();
        self.product.general_error = None;
        self.product.form_id = None;
        self.product.screen = origin;
        self.request_scroll_reset();
        self.refresh_product_list();
        self.reload_selected_product();
    }

    /// Ürün kodu alanı için kullanılmamış bir kod önerir.
    pub fn suggest_product_code(&mut self) {
        let code = match &self.database {
            Some(database) => match database.product_next_code() {
                Ok(code) => code,
                Err(error) => {
                    self.product.general_error = Some(error.to_string());
                    return;
                }
            },
            None => return,
        };

        self.product.draft.code = code;
        self.product.errors.clear(Field::Code);
    }

    /// Ayrıntı ekranını açar (ürün bilgisi ve stok hareketleri).
    pub fn open_product_detail(&mut self, id: i64) {
        match self.load_product(id) {
            Ok(product) => {
                self.product.selected = Some(product);
                self.product.detail_id = Some(id);
                self.product.screen = ProductScreen::Detail;
                self.product.errors = FieldErrors::default();
                self.product.movement_errors = FieldErrors::default();
                self.product.general_error = None;
                self.product.movement_confirmation = None;
                self.request_scroll_reset();
                self.reload_movements();
            }
            Err(message) => {
                self.product.general_error = Some(message);
                self.open_product_list();
            }
        }
    }

    /// Formdaki veriyi doğrulayıp kaydeder.
    pub fn submit_product_form(&mut self) {
        if self.product.form_id.is_some() && !self.product_form_dirty() {
            self.product.notice = Some("Kaydedilecek değişiklik yok.".to_owned());
            return;
        }

        let validated = match self.product.draft.validate() {
            Ok(validated) => validated,
            Err(errors) => {
                let focus = if errors.get(Field::Code).is_some() {
                    FocusTarget::ProductCode
                } else {
                    FocusTarget::ProductName
                };
                self.product.general_error = None;
                self.product.errors = errors;
                self.request_focus(focus);
                return;
            }
        };

        self.product.errors = FieldErrors::default();
        self.product.general_error = None;

        match self.persist_product(&validated) {
            Ok(id) => {
                self.product.form_id = None;
                self.product.notice = Some("Ürün kaydedildi.".to_owned());
                self.refresh_product_list();
                self.open_product_detail(id);
            }
            Err(SaveError::CodeTaken(code)) => {
                self.product.errors.push(
                    Field::Code,
                    format!("'{code}' ürün kodu zaten kullanılıyor. Lütfen başka bir kod girin."),
                );
                self.request_focus(FocusTarget::ProductCode);
            }
            Err(SaveError::BarcodeTaken(barcode)) => {
                self.product.errors.push(
                    Field::Barcode,
                    format!("'{barcode}' barkodu başka bir üründe kullanılıyor."),
                );
                self.request_focus(FocusTarget::ProductName);
            }
            Err(SaveError::General(message)) => {
                self.product.general_error = Some(message);
            }
        }
    }

    /// Ürünü aktif <-> pasif olarak değiştirir.
    pub fn toggle_product_active(&mut self, id: i64) {
        let database = match &self.database {
            Some(database) => database,
            None => {
                self.product.general_error =
                    Some("Veritabanı bağlantısı yok; durum değiştirilemedi.".to_owned());
                return;
            }
        };

        let outcome = match database.product_get(id) {
            Ok(Some(product)) => {
                let target = !product.active;
                database.product_set_active(id, target).map(|()| target)
            }
            Ok(None) => Err(AppError::ProductNotFound(id)),
            Err(error) => Err(error),
        };

        match outcome {
            Ok(target) => {
                self.product.notice = Some(
                    if target {
                        "Ürün aktif duruma getirildi."
                    } else {
                        "Ürün pasife alındı. Kayıt silinmedi."
                    }
                    .to_owned(),
                );
                self.product.general_error = None;
                self.refresh_product_list();
                self.reload_selected_product();
            }
            Err(error) => self.product.general_error = Some(error.to_string()),
        }
    }

    /// Bilgilendirme mesajını kapatır.
    pub fn dismiss_product_notice(&mut self) {
        self.product.notice = None;
    }

    /// Yeni stok hareketi formunu açar.
    pub fn open_movement_form(&mut self, product_id: i64) {
        if self.product.selected.as_ref().map(|product| product.id) != Some(product_id) {
            match self.load_product(product_id) {
                Ok(product) => self.product.selected = Some(product),
                Err(message) => {
                    self.product.general_error = Some(message);
                    self.open_product_list();
                    return;
                }
            }
        }

        self.product.detail_id = Some(product_id);
        self.product.movement_draft = MovementDraft::default();
        self.product.movement_original = self.product.movement_draft.clone();
        self.product.movement_errors = FieldErrors::default();
        self.product.movement_confirmation = None;
        self.product.general_error = None;
        self.product.screen = ProductScreen::MovementForm;
        self.request_scroll_reset();
        self.request_focus(FocusTarget::MovementQuantity);
    }

    /// Hareket formunu kaydetmeden kapatıp ürün ayrıntısına döner.
    pub fn cancel_movement_form(&mut self) {
        self.product.movement_errors = FieldErrors::default();
        self.product.movement_confirmation = None;
        self.product.general_error = None;
        self.product.screen = ProductScreen::Detail;
        self.request_scroll_reset();
    }

    /// Hareket kaydedilirse oluşacak yeni stoğu döndürür (önizleme).
    ///
    /// Miktar geçersizse ya da seçili ürün yoksa `None` döner.
    pub fn movement_preview(&self) -> Option<Quantity> {
        let product = self.product.selected.as_ref()?;
        let delta = self.product.movement_draft.delta()?;
        product.current_stock.checked_add(delta).ok()
    }

    /// Hareket formunu doğrulayıp kaydeder.
    ///
    /// Stok eksiye düşecekse kaydetmez; bunun yerine onay için bekleyen yeni
    /// stoğu ([`ProductState::movement_confirmation`]) saklar.
    pub fn submit_movement_form(&mut self) {
        let validated = match self.product.movement_draft.validate() {
            Ok(validated) => validated,
            Err(errors) => {
                self.product.general_error = None;
                self.product.movement_errors = errors;
                self.request_focus(FocusTarget::MovementQuantity);
                return;
            }
        };

        self.product.movement_errors = FieldErrors::default();
        self.product.general_error = None;

        let Some(product) = self.product.selected.as_ref() else {
            self.product.general_error = Some("Ürün bilgisi bulunamadı.".to_owned());
            return;
        };

        match product.current_stock.checked_add(validated.quantity_delta) {
            Ok(new_stock) if new_stock.is_negative() => {
                self.product.movement_confirmation = Some(new_stock);
            }
            Ok(_) => self.persist_movement(&validated),
            Err(error) => self.product.general_error = Some(error.to_string()),
        }
    }

    /// Negatif stok onayını verir ve hareketi kaydeder.
    pub fn confirm_pending_movement(&mut self) {
        self.product.movement_confirmation = None;

        match self.product.movement_draft.validate() {
            Ok(validated) => self.persist_movement(&validated),
            Err(errors) => self.product.movement_errors = errors,
        }
    }

    /// Negatif stok onayını iptal eder (hareket kaydedilmez).
    pub fn cancel_pending_movement(&mut self) {
        self.product.movement_confirmation = None;
    }

    /// Genel Bakış ekranı için gerçek verilerden özet üretir.
    ///
    /// Veritabanı yoksa veya okuma başarısız olursa ilgili değer sıfır kalır;
    /// sahte finansal tutar **üretilmez**.
    pub fn overview_metrics(&self) -> OverviewMetrics {
        let Some(database) = &self.database else {
            return OverviewMetrics::default();
        };

        OverviewMetrics {
            active_products: database.active_product_count().unwrap_or(0),
            low_stock_products: database.low_stock_count().unwrap_or(0),
            customers: database.customer_count().unwrap_or(0),
            recent_movements: database.recent_movements(5).unwrap_or_default(),
            sales_total: database.confirmed_sales_total().unwrap_or_default(),
            purchase_total: database.confirmed_purchases_total().unwrap_or_default(),
            recent_invoices: database.recent_invoices(5).unwrap_or_default(),
        }
    }

    /// Kaydı kaydeder (ekleme veya güncelleme).
    fn persist_product(&self, product: &ValidatedProduct) -> Result<i64, SaveError> {
        let Some(database) = &self.database else {
            return Err(SaveError::General(
                "Veritabanı bağlantısı yok; kayıt yapılamadı.".to_owned(),
            ));
        };

        let form_id = self.product.form_id;

        match database.product_code_exists(&product.code, form_id) {
            Ok(true) => return Err(SaveError::CodeTaken(product.code.clone())),
            Ok(false) => {}
            Err(error) => return Err(SaveError::General(error.to_string())),
        }

        if let Some(barcode) = &product.barcode {
            match database.product_barcode_exists(barcode, form_id) {
                Ok(true) => return Err(SaveError::BarcodeTaken(barcode.clone())),
                Ok(false) => {}
                Err(error) => return Err(SaveError::General(error.to_string())),
            }
        }

        let result = match form_id {
            Some(id) => database.product_update(id, product).map(|()| id),
            None => database.product_insert(product),
        };

        match result {
            Ok(id) => Ok(id),
            Err(AppError::ProductCodeTaken(code)) => Err(SaveError::CodeTaken(code)),
            Err(AppError::ProductBarcodeTaken(barcode)) => Err(SaveError::BarcodeTaken(barcode)),
            Err(error) => Err(SaveError::General(error.to_string())),
        }
    }

    /// Hareketi veritabanına yazar ve ayrıntı ekranını tazeler.
    fn persist_movement(&mut self, movement: &ValidatedMovement) {
        let Some(product_id) = self.product.detail_id else {
            self.product.general_error = Some("Ürün bilgisi bulunamadı.".to_owned());
            return;
        };

        let Some(database) = &self.database else {
            self.product.general_error =
                Some("Veritabanı bağlantısı yok; hareket kaydedilemedi.".to_owned());
            return;
        };

        match database.movement_insert(product_id, movement) {
            Ok(_) => {
                self.product.movement_confirmation = None;
                self.product.movement_draft = MovementDraft::default();
                self.product.movement_errors = FieldErrors::default();
                self.product.notice = Some("Stok hareketi kaydedildi.".to_owned());
                self.product.general_error = None;
                self.product.screen = ProductScreen::Detail;
                self.request_scroll_reset();
                self.reload_selected_product();
                self.refresh_product_list();
            }
            Err(error) => self.product.general_error = Some(error.to_string()),
        }
    }

    /// Kimliğine göre kaydı okur; bulunamazsa kullanıcıya gösterilecek mesaj döner.
    fn load_product(&self, id: i64) -> Result<Product, String> {
        let Some(database) = &self.database else {
            return Err("Veritabanı bağlantısı yok; kayıt okunamadı.".to_owned());
        };

        match database.product_get(id) {
            Ok(Some(product)) => Ok(product),
            Ok(None) => Err(format!("Ürün bulunamadı (kayıt no: {id}).")),
            Err(error) => Err(error.to_string()),
        }
    }

    /// Ayrıntı ekranındaki kaydı ve hareketleri veritabanından tazeler.
    fn reload_selected_product(&mut self) {
        let Some(id) = self.product.detail_id else {
            self.product.selected = None;
            self.product.movements = Vec::new();
            return;
        };

        let Some(database) = &self.database else {
            return;
        };

        self.product.selected = database.product_get(id).ok().flatten();
        self.product.movements = database.movement_list_for_product(id).unwrap_or_default();
    }

    /// Ayrıntı ekranındaki stok hareketlerini tazeler.
    fn reload_movements(&mut self) {
        let Some(id) = self.product.detail_id else {
            self.product.movements = Vec::new();
            return;
        };

        self.product.movements = match &self.database {
            Some(database) => database.movement_list_for_product(id).unwrap_or_default(),
            None => Vec::new(),
        };
    }
}
