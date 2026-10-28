// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Cari hesap ekranının durumu ve komutları.
//!
//! Arayüz katmanı cari hesaplarla ilgili **hiçbir SQL çalıştırmaz**. Ekranın
//! gösterdiği tüm veri [`CustomerState`] içinde tutulur ve tüm değişiklikler
//! bu modüldeki komut metotları (`open_customer_new_form`, `submit_customer_form`,
//! `toggle_customer_active`, ...) ile yapılır.
//!
//! ## Neden durum burada?
//!
//! Form alanları, filtre ve seçim bilgisi kareler (frame) arasında korunmalıdır.
//! Bu bilgiyi arayüz modülünde tutmak, veritabanı bağlantısına erişimi olmayan
//! bir katmanda kalıcı durum oluştururdu. Bu nedenle durum uygulama katmanında
//! tutulur; arayüz yalnızca okur ve kullanıcı eylemlerini komutlara çevirir.
//!
//! ## Kaydetme akışı
//!
//! ```text
//! Form (CustomerDraft) --validate--> ValidatedCustomer --SQL--> customers tablosu
//!        ^                                    |
//!        |  hata: FieldErrors (girdi korunur)  |  hata: kullanıcıya mesaj
//!        +------------------------------------+
//! ```
//!
//! Kaydetme başarısız olursa (doğrulama veya veritabanı hatası) kullanıcının
//! girdiği bilgiler **silinmez**; form ekranında kalır.

use crate::customer::{
    Customer, CustomerDraft, CustomerFilter, Field, FieldErrors, ValidatedCustomer,
};
use crate::error::AppError;

use super::{FocusTarget, OnMuhasebeApp};

/// Cari hesap bölümünde gösterilen alt ekranlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CustomerScreen {
    /// Kayıt listesi (arama ve filtreleme ile).
    #[default]
    List,
    /// Yeni kayıt / düzenleme formu.
    Form,
    /// Kayıt ayrıntısı ve hesap özeti.
    Detail,
}

/// Cari hesap bölümünün çalışma zamanı durumu.
#[derive(Debug)]
pub struct CustomerState {
    /// Gösterilen alt ekran.
    pub screen: CustomerScreen,
    /// Ayrıntısı gösterilen kaydın kimliği.
    pub detail_id: Option<i64>,
    /// Düzenlenen kaydın kimliği (`None` = yeni kayıt).
    pub form_id: Option<i64>,
    /// Form iptal edildiğinde dönülecek ekran.
    pub form_origin: CustomerScreen,
    /// Formdaki ham kullanıcı girdisi.
    pub draft: CustomerDraft,
    /// Form açıldığındaki başlangıç değerleri (değişiklik algılama).
    pub form_original: CustomerDraft,
    /// Alan bazlı doğrulama hataları.
    pub errors: FieldErrors,
    /// Liste ölçütleri (arama, tür, durum).
    pub filter: CustomerFilter,
    /// Ölçütlere uyan kayıtlar.
    pub list: Vec<Customer>,
    /// Liste yüklenirken oluşan hata.
    pub list_error: Option<String>,
    /// Ayrıntı ekranında gösterilen kayıt.
    pub selected: Option<Customer>,
    /// Son başarılı işlemin bilgilendirmesi.
    pub notice: Option<String>,
    /// Son başarısız işlemin hata mesajı.
    pub general_error: Option<String>,
}

impl Default for CustomerState {
    fn default() -> Self {
        Self {
            screen: CustomerScreen::List,
            detail_id: None,
            form_id: None,
            form_origin: CustomerScreen::List,
            draft: CustomerDraft::default(),
            form_original: CustomerDraft::default(),
            errors: FieldErrors::default(),
            filter: CustomerFilter::default(),
            list: Vec::new(),
            list_error: None,
            selected: None,
            notice: None,
            general_error: None,
        }
    }
}

/// Kaydetme sırasında oluşabilecek hatalar.
///
/// Cari kodu çakışması alan hatası olarak gösterilir (form açık kalır), diğer
/// tüm sorunlar genel hata mesajı olarak gösterilir.
enum SaveError {
    /// Cari kodu başka bir kayıtta kullanılıyor.
    CodeTaken(String),
    /// Diğer hatalar (veritabanı, bağlantı yok, ...).
    General(String),
}

impl OnMuhasebeApp {
    /// Cari hesap durumunu okur.
    pub fn customer_state(&self) -> &CustomerState {
        &self.customer
    }

    /// Cari hesap durumunu değiştirilebilir olarak verir (form alanları).
    pub fn customer_state_mut(&mut self) -> &mut CustomerState {
        &mut self.customer
    }

    /// Cari formunda kaydedilmemiş değişiklik var mı?
    pub fn customer_form_dirty(&self) -> bool {
        self.customer.screen == CustomerScreen::Form
            && self.customer.draft != self.customer.form_original
    }

    /// Kayıt listesini geçerli ölçütlere göre veritabanından yeniden okur.
    pub fn refresh_customer_list(&mut self) {
        self.customer.list_error = None;

        let Some(database) = &self.database else {
            self.customer.list = Vec::new();
            self.customer.list_error = Some(
                "Veritabanı bağlantısı kurulamadığı için cari hesaplar yüklenemedi.".to_owned(),
            );
            return;
        };

        match database.customer_list(&self.customer.filter) {
            Ok(customers) => self.customer.list = customers,
            Err(error) => {
                self.customer.list = Vec::new();
                self.customer.list_error = Some(error.to_string());
            }
        }
    }

    /// Arama ve filtreleri temizler ve listeyi yeniler.
    pub fn reset_customer_filter(&mut self) {
        self.customer.filter = CustomerFilter::default();
        self.refresh_customer_list();
    }

    /// Liste ekranına döner.
    pub fn open_customer_list(&mut self) {
        self.customer.screen = CustomerScreen::List;
        self.customer.errors = FieldErrors::default();
        self.customer.general_error = None;
        self.request_scroll_reset();
        self.refresh_customer_list();
    }

    /// Yeni kayıt formunu açar; cari kodu önerilir.
    pub fn open_customer_new_form(&mut self) {
        self.customer.draft = CustomerDraft::default();
        self.customer.errors = FieldErrors::default();
        self.customer.general_error = None;
        self.customer.form_id = None;
        self.customer.form_origin = self.customer.screen;
        self.customer.screen = CustomerScreen::Form;
        self.request_scroll_reset();
        self.suggest_customer_code();
        self.customer.form_original = self.customer.draft.clone();
        self.request_focus(FocusTarget::CustomerCode);
    }

    /// Var olan kaydı düzenlemek üzere formu açar.
    pub fn open_customer_edit_form(&mut self, id: i64) {
        match self.load_customer(id) {
            Ok(customer) => {
                self.customer.draft = CustomerDraft::from_customer(&customer);
                self.customer.errors = FieldErrors::default();
                self.customer.general_error = None;
                self.customer.form_id = Some(customer.id);
                self.customer.form_origin = self.customer.screen;
                self.customer.screen = CustomerScreen::Form;
                self.customer.form_original = self.customer.draft.clone();
                self.request_scroll_reset();
                self.request_focus(FocusTarget::CustomerCode);
            }
            Err(message) => {
                self.customer.general_error = Some(message);
                self.customer.screen = CustomerScreen::List;
                self.refresh_customer_list();
            }
        }
    }

    /// Formu kaydetmeden kapatır ve geldiği ekrana döner.
    ///
    /// Kullanıcı girdisi bilinçli olarak temizlenir: kaydetmeden çıkılan form
    /// yeniden açıldığında güncel kayıtla başlar.
    pub fn cancel_customer_form(&mut self) {
        let origin = self.customer.form_origin;
        self.customer.errors = FieldErrors::default();
        self.customer.general_error = None;
        self.customer.form_id = None;
        self.customer.screen = origin;
        self.request_scroll_reset();
        self.refresh_customer_list();
        self.reload_selected_customer();
    }

    /// Cari kodu alanı için kullanılmamış bir kod önerir.
    pub fn suggest_customer_code(&mut self) {
        let code = match &self.database {
            Some(database) => match database.customer_next_code() {
                Ok(code) => code,
                Err(error) => {
                    self.customer.general_error = Some(error.to_string());
                    return;
                }
            },
            None => return,
        };

        self.customer.draft.code = code;
        self.customer.errors.clear(Field::Code);
    }

    /// Ayrıntı ekranını açar.
    pub fn open_customer_detail(&mut self, id: i64) {
        match self.load_customer(id) {
            Ok(customer) => {
                self.customer.selected = Some(customer);
                self.customer.detail_id = Some(id);
                self.customer.screen = CustomerScreen::Detail;
                self.customer.errors = FieldErrors::default();
                self.customer.general_error = None;
                self.request_scroll_reset();
            }
            Err(message) => {
                self.customer.general_error = Some(message);
                self.customer.screen = CustomerScreen::List;
                self.refresh_customer_list();
            }
        }
    }

    /// Formdaki veriyi doğrulayıp kaydeder.
    ///
    /// Doğrulama başarısızsa alan hataları doldurulur ve **kullanıcı girdisi
    /// korunur**. Kaydetme başarılıysa kaydın ayrıntı ekranı açılır ve
    /// bilgilendirme mesajı gösterilir. Veritabanı hatası oluşursa form ekranı
    /// ve girilen bilgiler olduğu gibi kalır.
    pub fn submit_customer_form(&mut self) {
        if self.customer.form_id.is_some() && !self.customer_form_dirty() {
            self.customer.notice = Some("Kaydedilecek değişiklik yok.".to_owned());
            return;
        }

        let validated = match self.customer.draft.validate() {
            Ok(validated) => validated,
            Err(errors) => {
                let focus = if errors.get(Field::Code).is_some() {
                    FocusTarget::CustomerCode
                } else {
                    FocusTarget::CustomerName
                };
                self.customer.general_error = None;
                self.customer.errors = errors;
                self.request_focus(focus);
                return;
            }
        };

        self.customer.errors = FieldErrors::default();
        self.customer.general_error = None;

        match self.persist_customer(&validated) {
            Ok(id) => {
                self.customer.form_id = None;
                self.customer.notice = Some("Cari hesap kaydedildi.".to_owned());
                self.refresh_customer_list();
                self.open_customer_detail(id);
            }
            Err(SaveError::CodeTaken(code)) => {
                self.customer.errors.push(
                    Field::Code,
                    format!("'{code}' cari kodu zaten kullanılıyor. Lütfen başka bir kod girin."),
                );
                self.request_focus(FocusTarget::CustomerCode);
            }
            Err(SaveError::General(message)) => {
                self.customer.general_error = Some(message);
            }
        }
    }

    /// Kaydın durumunu aktif <-> pasif olarak değiştirir.
    ///
    /// Kayıt **silinmez**; ileride bağlanacak hareketlerin geçmişi korunur.
    pub fn toggle_customer_active(&mut self, id: i64) {
        let database = match &self.database {
            Some(database) => database,
            None => {
                self.customer.general_error =
                    Some("Veritabanı bağlantısı yok; durum değiştirilemedi.".to_owned());
                return;
            }
        };

        let outcome = match database.customer_get(id) {
            Ok(Some(customer)) => {
                let target = !customer.active;
                database.customer_set_active(id, target).map(|()| target)
            }
            Ok(None) => Err(AppError::CustomerNotFound(id)),
            Err(error) => Err(error),
        };

        match outcome {
            Ok(target) => {
                self.customer.notice = Some(
                    if target {
                        "Cari hesap aktif duruma getirildi."
                    } else {
                        "Cari hesap pasife alındı. Kayıt silinmedi."
                    }
                    .to_owned(),
                );
                self.customer.general_error = None;
                self.refresh_customer_list();
                self.reload_selected_customer();
            }
            Err(error) => self.customer.general_error = Some(error.to_string()),
        }
    }

    /// Bilgilendirme mesajını kapatır.
    pub fn dismiss_customer_notice(&mut self) {
        self.customer.notice = None;
    }

    /// Kaydı kaydeder (ekleme veya güncelleme).
    ///
    /// Kod benzersizliği için önden kontrol yapılır; asıl güvence veritabanındaki
    /// `UNIQUE` kısıtıdır ve oradan dönen ihlal de aynı hataya çevrilir. Böylece
    /// araya giren başka bir kayıt olsa bile çakışma sessizce geçmez.
    fn persist_customer(&self, customer: &ValidatedCustomer) -> Result<i64, SaveError> {
        let Some(database) = &self.database else {
            return Err(SaveError::General(
                "Veritabanı bağlantısı yok; kayıt yapılamadı.".to_owned(),
            ));
        };

        let form_id = self.customer.form_id;

        match database.customer_code_exists(&customer.code, form_id) {
            Ok(true) => return Err(SaveError::CodeTaken(customer.code.clone())),
            Ok(false) => {}
            Err(error) => return Err(SaveError::General(error.to_string())),
        }

        let result = match form_id {
            Some(id) => database.customer_update(id, customer).map(|()| id),
            None => database.customer_insert(customer),
        };

        match result {
            Ok(id) => Ok(id),
            Err(AppError::CustomerCodeTaken(code)) => Err(SaveError::CodeTaken(code)),
            Err(error) => Err(SaveError::General(error.to_string())),
        }
    }

    /// Kimliğine göre kaydı okur; bulunamazsa kullanıcıya gösterilecek mesaj döner.
    fn load_customer(&self, id: i64) -> Result<Customer, String> {
        let Some(database) = &self.database else {
            return Err("Veritabanı bağlantısı yok; kayıt okunamadı.".to_owned());
        };

        match database.customer_get(id) {
            Ok(Some(customer)) => Ok(customer),
            Ok(None) => Err(format!("Cari hesap bulunamadı (kayıt no: {id}).")),
            Err(error) => Err(error.to_string()),
        }
    }

    /// Ayrıntı ekranındaki kaydı veritabanından tazeler.
    fn reload_selected_customer(&mut self) {
        let Some(id) = self.customer.detail_id else {
            self.customer.selected = None;
            return;
        };

        self.customer.selected = match &self.database {
            Some(database) => database.customer_get(id).ok().flatten(),
            None => None,
        };
    }
}
