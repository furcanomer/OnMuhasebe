// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Günlük (log) altyapısı: günlük dosya, 10 MB rollover, 30 gün temizliği,
//! çalışma zamanında seviye değiştirme ve panik kaydı.
//!
//! Dosyalar veri dizinindeki `logs` klasöründe
//! `onmuhasebe-YYYY-MM-DD.log` (gerekirse `.1.log`, `.2.log`) adıyla tutulur.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{Local, NaiveDate, TimeDelta};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Registry;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::reload;

use crate::config::AppConfig;
use crate::error::{AppError, Result};

/// Tek bir günlük dosyasının üst sınırı (10 MB).
pub const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

/// Günlük dosyalarının saklanma süresi (gün).
pub const LOG_RETENTION_DAYS: i64 = 30;

type LevelHandle = reload::Handle<EnvFilter, Registry>;

static LEVEL_HANDLE: OnceLock<Mutex<Option<LevelHandle>>> = OnceLock::new();
static PANIC_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Log paketleyicisinin yaşam süresini tutan guard. `main` boyunca saklanır.
pub struct LoggingGuard {
    _worker: tracing_appender::non_blocking::WorkerGuard,
}

/// Günlük + boyut tabanlı dosya yazıcısı.
#[derive(Clone)]
pub struct DailySizeWriter {
    directory: PathBuf,
    max_bytes: u64,
    state: Arc<Mutex<WriterState>>,
}

struct WriterState {
    date: NaiveDate,
    part: u32,
    file: File,
    size: u64,
}

impl DailySizeWriter {
    /// Verilen klasörde yazıcı oluşturur.
    pub fn new(directory: impl Into<PathBuf>, max_bytes: u64) -> Result<Self> {
        let directory = directory.into();
        fs::create_dir_all(&directory).map_err(|error| AppError::file_system(&directory, error))?;
        let date = Local::now().date_naive();
        let (part, file, size) = open_current(&directory, date, max_bytes)
            .map_err(|error| AppError::file_system(&directory, error))?;
        Ok(Self {
            directory,
            max_bytes: max_bytes.max(1),
            state: Arc::new(Mutex::new(WriterState {
                date,
                part,
                file,
                size,
            })),
        })
    }

    /// Bugüne ait dosya yolu (test/gösterim için).
    pub fn current_path(&self) -> PathBuf {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        log_path(&self.directory, state.date, state.part)
    }
}

fn log_path(directory: &Path, date: NaiveDate, part: u32) -> PathBuf {
    if part == 0 {
        directory.join(format!("onmuhasebe-{}.log", date.format("%Y-%m-%d")))
    } else {
        directory.join(format!(
            "onmuhasebe-{}.{}.log",
            date.format("%Y-%m-%d"),
            part
        ))
    }
}

fn open_file(path: &Path) -> io::Result<(File, u64)> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let size = file.metadata()?.len();
    Ok((file, size))
}

fn open_current(directory: &Path, date: NaiveDate, max_bytes: u64) -> io::Result<(u32, File, u64)> {
    let mut part = 0;
    loop {
        let path = log_path(directory, date, part);
        let (file, size) = open_file(&path)?;
        if size < max_bytes {
            return Ok((part, file, size));
        }
        part = part.saturating_add(1);
    }
}

impl Write for DailySizeWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let today = Local::now().date_naive();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        if state.date != today {
            let (part, file, size) = open_current(&self.directory, today, self.max_bytes)?;
            state.date = today;
            state.part = part;
            state.file = file;
            state.size = size;
        }

        if !buffer.is_empty()
            && state.size > 0
            && state.size.saturating_add(buffer.len() as u64) > self.max_bytes
        {
            state.part = state.part.saturating_add(1);
            let path = log_path(&self.directory, state.date, state.part);
            let (file, size) = open_file(&path)?;
            state.file = file;
            state.size = size;
        }

        state.file.write_all(buffer)?;
        state.size = state.size.saturating_add(buffer.len() as u64);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .file
            .flush()
    }
}

/// Günlük altyapısını başlatır. Yalnızca bir kez çağrılmalıdır.
pub fn init(config: &AppConfig, info_enabled: bool) -> Result<LoggingGuard> {
    let directory = config.logs_dir();
    cleanup_old_logs(&directory, Local::now().date_naive(), LOG_RETENTION_DAYS)?;

    let writer = DailySizeWriter::new(&directory, MAX_LOG_BYTES)?;
    let (non_blocking, worker) = tracing_appender::non_blocking(writer);
    let filter = EnvFilter::new(level_expression(info_enabled));
    let (reload_layer, handle) = reload::Layer::new(filter);

    tracing_subscriber::registry()
        .with(reload_layer)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .try_init()
        .map_err(|error| AppError::Maintenance(format!("Log altyapısı başlatılamadı: {error}")))?;

    let slot = LEVEL_HANDLE.get_or_init(|| Mutex::new(None));
    *slot.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(handle);
    Ok(LoggingGuard { _worker: worker })
}

fn level_expression(info_enabled: bool) -> &'static str {
    if info_enabled { "info" } else { "warn" }
}

/// Çalışma zamanında log seviyesini değiştirir (`true`: info, `false`: warn).
pub fn set_info_enabled(info_enabled: bool) {
    let Some(slot) = LEVEL_HANDLE.get() else {
        return;
    };
    if let Some(handle) = slot
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .as_ref()
    {
        let _ = handle.modify(|filter| *filter = EnvFilter::new(level_expression(info_enabled)));
    }
}

/// Günlük klasörünü döndürür (Yardım > Log klasörünü aç için).
pub fn logs_dir(config: &AppConfig) -> PathBuf {
    config.logs_dir()
}

/// Klasördeki en yeni günlük dosyasını döndürür.
pub fn latest_log_file(directory: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(directory).ok()?;
    let mut files: Vec<(NaiveDate, u32, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let (date, part) = parse_log_file(path.file_name()?.to_str()?)?;
            Some((date, part, path))
        })
        .collect();
    files.sort_by_key(|(date, part, _)| (std::cmp::Reverse(*date), std::cmp::Reverse(*part)));
    files.into_iter().next().map(|(_, _, path)| path)
}

fn parse_log_file(name: &str) -> Option<(NaiveDate, u32)> {
    let stem = name.strip_prefix("onmuhasebe-")?.strip_suffix(".log")?;
    let mut pieces = stem.split('.');
    let date = NaiveDate::parse_from_str(pieces.next()?, "%Y-%m-%d").ok()?;
    let part = pieces
        .next()
        .and_then(|part| part.parse().ok())
        .unwrap_or(0);
    Some((date, part))
}

/// 30 günden eski günlük dosyalarını siler. Testlerde tarih enjekte edilebilir.
pub fn cleanup_old_logs(directory: &Path, today: NaiveDate, keep_days: i64) -> Result<usize> {
    if !directory.exists() {
        return Ok(0);
    }
    let cutoff = today - TimeDelta::days(keep_days.max(0));
    let mut removed = 0;
    for entry in fs::read_dir(directory).map_err(|error| AppError::file_system(directory, error))? {
        let entry = entry.map_err(|error| AppError::file_system(directory, error))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some((date, _)) = parse_log_file(name) else {
            continue;
        };
        if date < cutoff {
            let path = entry.path();
            fs::remove_file(&path).map_err(|error| AppError::file_system(&path, error))?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Panik mesajını, konumunu, thread adını ve istenirse backtrace'i loglar.
pub fn panic_record(
    message: &str,
    location: &str,
    thread: &str,
    backtrace: Option<&str>,
) -> String {
    match backtrace {
        Some(backtrace) => {
            format!("PANİK: {message} | konum: {location} | thread: {thread}\n{backtrace}")
        }
        None => format!("PANİK: {message} | konum: {location} | thread: {thread}"),
    }
}

/// Panikleri loglayan özel hook'u kurar; önceki hook'u da çalıştırır.
pub fn install_panic_hook() {
    if PANIC_HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|value| (*value).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "bilinmeyen panik".to_owned());
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "bilinmeyen konum".to_owned());
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("isimsiz");
        let backtrace = std::env::var_os("RUST_BACKTRACE")
            .filter(|value| value != "0")
            .map(|_| std::backtrace::Backtrace::force_capture().to_string());
        tracing::error!(
            target: "panic",
            location = %location,
            thread = %thread_name,
            "{}",
            panic_record(&message, &location, thread_name, backtrace.as_deref())
        );
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gunluk_dosya_yazilir_ve_boyutta_rollover_olur() {
        let directory = tempfile::tempdir().unwrap();
        let mut writer = DailySizeWriter::new(directory.path(), 20).unwrap();
        writer.write_all(b"birinci kayit").unwrap();
        writer.write_all(b"ikinci-kayit-buyuk").unwrap();
        writer.flush().unwrap();

        let files: Vec<PathBuf> = fs::read_dir(directory.path())
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .collect();
        assert!(files.len() >= 2, "10 MB/20 bayt sınırında rollover olmalı");
        assert!(
            files
                .iter()
                .any(|path| path.to_string_lossy().contains(".1.log"))
        );
    }

    #[test]
    fn otuz_gunden_eski_loglar_silinir() {
        let directory = tempfile::tempdir().unwrap();
        for name in [
            "onmuhasebe-2026-08-01.log",
            "onmuhasebe-2026-09-01.2.log",
            "onmuhasebe-2026-10-23.log",
            "baska.log",
        ] {
            fs::write(directory.path().join(name), b"x").unwrap();
        }
        let removed = cleanup_old_logs(
            directory.path(),
            NaiveDate::from_ymd_opt(2026, 10, 23).unwrap(),
            30,
        )
        .unwrap();
        assert_eq!(removed, 2);
        assert!(!directory.path().join("onmuhasebe-2026-08-01.log").exists());
        assert!(
            !directory
                .path()
                .join("onmuhasebe-2026-09-01.2.log")
                .exists()
        );
        assert!(directory.path().join("onmuhasebe-2026-10-23.log").exists());
        assert!(directory.path().join("baska.log").exists());
    }

    #[test]
    fn panik_kaydi_konum_ve_thread_icerir() {
        let record = panic_record("deneme", "src/main.rs:10", "ana", None);
        assert!(record.contains("PANİK: deneme"));
        assert!(record.contains("src/main.rs:10"));
        assert!(record.contains("ana"));
    }

    #[test]
    fn log_dosyasi_adi_cozumlenir() {
        assert_eq!(
            parse_log_file("onmuhasebe-2026-10-23.2.log"),
            Some((NaiveDate::from_ymd_opt(2026, 10, 23).unwrap(), 2))
        );
        assert_eq!(parse_log_file("onmuhasebe-eski.log"), None);
    }

    #[test]
    fn en_yeni_log_dosyasi_bulunur() {
        let directory = tempfile::tempdir().unwrap();
        for name in [
            "onmuhasebe-2026-10-22.log",
            "onmuhasebe-2026-10-23.log",
            "onmuhasebe-2026-10-23.1.log",
            "onmuhasebe-2026-10-23.10.log",
            "baska.log",
        ] {
            fs::write(directory.path().join(name), b"x").unwrap();
        }
        assert_eq!(
            latest_log_file(directory.path())
                .unwrap()
                .file_name()
                .unwrap(),
            "onmuhasebe-2026-10-23.10.log"
        );
    }
}
