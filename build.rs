// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Windows yürütülebilir dosyasına sürüm ve telif bilgilerini gömer.
//!
//! Bu betik yalnızca Windows hedefinde çalışır. `winres` derleme zamanı
//! bağımlılığıdır; üretilen `.res` dosyası exe'ye bağlanır ve çalışma zamanında
//! ek bir yük oluşturmaz.

fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set("CompanyName", "@furcanomer");
        res.set("FileDescription", "Ön Muhasebe");
        res.set("ProductName", "Ön Muhasebe");
        res.set(
            "LegalCopyright",
            "Copyright (c) 2026 @furcanomer. BSD 3-Clause License.",
        );
        res.set("OriginalFilename", "OnMuhasebe.exe");
        res.set("InternalName", "OnMuhasebe");
        if let Ok(v) = std::env::var("CARGO_PKG_VERSION") {
            res.set("ProductVersion", &v);
            res.set("FileVersion", &v);
        }
        res.set_icon("assets/icon.ico");
        res.compile().expect("winres derlemesi basarisiz");
    }
}
