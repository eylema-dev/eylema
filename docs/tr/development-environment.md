# Native geliştirme ortamı — G04

Kaynak revizyon 0.1.0. [English](../en/development-environment.md). Bu, tek fiziksel Mac için geliştirme aracı kanıtıdır; uygulanmış Eylema uygulaması değildir. Belge aşaması kaynak engeli etkin kalır; G07 kök workspace, toolchain dosyası ve gerçek uygulama CI’ını birlikte sağlar.

## Gözlenen ortam ve tekrarlanabilir denemeler

[Gözlenen JSON](../../tools/doctor/observed-macos.json) kesin sürümleri, komutları, doctor/örnek kaynak hash’lerini, çıktıları ve artifact hash’lerini bağlar. Geçici çıktılar kontrolden sonra silinir; yeniden derlemede binary hash değişebilir. Kaynak/araç/ortam kimliği ve davranış eski raporun passed etiketinden çıkarılmaz; yeniden doğrulanır.

| Girdi | Gözlem |
|---|---|
| Fiziksel cihaz | Tek Apple M4 Pro, 25.769.803.776 bayt birleşik bellek (24 GiB) |
| macOS | 27.0.1, ARM64 |
| Rust | 1.99.0, aarch64-apple-darwin, commit b940084d7eb6a299eb4bfeb8e34901bc051e7ac4 |
| Geliştirme araçları | Rustup 1.29.1, Cargo 1.99.0, rustfmt 1.10.0-stable, clippy 0.1.99 |
| Rust hedefleri | aarch64-apple-darwin ve wasm32-unknown-unknown |
| Apple SDK/araçları | Xcode 27.0 / 27A266a, macOS SDK 27.0, Metal 32023.921 |
| Tarayıcı envanteri | Chrome 154.0.8037.98; burada tarayıcı yürütümü veya WebGPU kabulü yok |

[Kendi doctor aracımız](../../tools/doctor/doctor.py) Python 3.14’ü yalnız geliştirme aracı olarak kullanır ve üç açık geçici birinci taraf deneme kaynağı üretir: bilinen mesajlı Rust executable, Wasm’a derlenen taşınabilir Rust fonksiyonu ve GPU’ya gönderilmeden derlenip linklenen Metal 4.0 kernel. Üretim crate’i veya çıkarım motoru eklemez. Native binary’nin ARM64 Mach-O türü ve gerçekten döndürdüğü mesaj kontrol edilir. Başarılı araç çağrısından sonra Wasm sürüm/header, dolu AIR ve metallib magic doğrulanır. Dört geliştirme aracının sürüm çıktısı sabit sürüm/build kimliğiyle tam eşleşmelidir. Yanlış/eksik araç, desteklenmeyen host/cihaz, beklenmeyen Rust commit/sürümü, eksik target, başarısız/zaman aşımına uğramış komut veya geçersiz çıktı doctor’ı başarısız yapar.

```sh
env -u RUST_LOG .venv/bin/python tools/doctor/doctor.py
env -u RUST_LOG .venv/bin/python tools/doctor/doctor.py --output /tmp/eylema-doctor.json
```

İlk çalıştırmada Codex host’undan RUST_LOG devralınmıştı; kabul edilmiş derleyici ortamı kontrolü bunu reddetti. İlgisiz günlük ayarını temizlemek kontrolü değiştirmez. Derleyici override’ının etkisini gizlemek için ayarı temizlemeyin; incelenmiş ortamla çalışın. Her komut 120 saniyeyle sınırlıdır. Hata sıfır olmayan kodla çıkar, failed sonuç yazdırır ve mevcut çıktı dosyasını korur. Home/deneme yolları ve geçici Metal mount dizinleri maskelenir; cihaz seri numarası, kimlik bilgisi ve özel eğitim verisi toplanmaz.

## Kurulum ve sınırlar

[Kurulum kaydı](../../tools/doctor/installation.json) resmî rustup installer sürüm/URL/SHA-256 ve istenen bileşenleri sabitler. Çalıştırmadan önce indirilen baytları resmî SHA ile doğrulayın. [Rustup kurulumu](https://rust-lang.github.io/rustup/installation/index.html) ve [sürümlü toolchain](https://rust-lang.github.io/rustup/concepts/toolchains.html) standart araçları ve kesin sürüm seçimini açıklar. Yerel kurulum shell profilini düzenlemedi; Rust komutları için standart Cargo bin dizinini mevcut shell’de etkinleştirin.

```sh
rustup toolchain install 1.99.0 --profile minimal --component rustfmt --component clippy --target wasm32-unknown-unknown
xcodebuild -downloadComponent MetalToolchain
```

Kurulum öncesi Metal compiler stub bulunabiliyor, fakat çalışmıyordu; metallib eksikti. Resmî bileşen kurulunca derleyici/linker çağrısı geçti. Apple’ın [ek bileşen rehberi](https://developer.apple.com/documentation/xcode/downloading-and-installing-additional-xcode-components) araç sınırını açıklar. Sudo, depo yetkisi, yayın veya ücretli servis kullanılmadı.

Passed: gerçek native Rust derleme/çalıştırma, Wasm derleme, Metal shader derleme/link ve yerel ortam envanteri. Not run: Eylema runtime/workspace, Wasm/tarayıcı yürütümü, GPU hesaplama/readback, model çıkarımı, eğitim, performans veya sürüm kabulü. Sekiz hata odaklı birim testi mock komutlar ve sentetik çıktılar kullanır; bunların başarısı doğrulayıcı kanıtıdır, gerçek yerel denemelerden ayrıdır. Çıktı değiştirme atomiktir; depolama hatası eski raporu korur. Hosted depo CI’ı bu testleri çalıştırır, belge Rust kapısını skipped bırakır; bu Mac’in ortam kabulünü tekrar çalıştırmaz. G07 ve G19 kendi gerçek uygulama/runtime/GPU kanıtını sağlamalıdır.
