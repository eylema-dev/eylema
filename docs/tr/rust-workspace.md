# Kendi Rust workspace başlangıcımız

Kaynak revizyon 0.1.0. İngilizce kanoniktir. [English](../en/rust-workspace.md).

G07 ilk gerçek Rust workspace’i getirir: bir taşınabilir eylema-contracts kütüphanesi ve onu kullanan bir native eylema host. İki paket de workspace içindeki kendi kodumuzdur, yayımlanmamıştır ve workspace dışında bağımlılıkları yoktur. Cargo.lock yalnız bu iki paketi içerir; üçüncü taraf üretim/geliştirme crate’i veya build script yoktur. Dağıtım lisansı henüz seçilmemiştir.

Contracts kütüphanesi şimdilik yalnız paket build kimliğini sunar. Native host help/version kabul eder; desteklenmeyen, fazla ve Unicode olmayan argümanları başarılı sonuç veya panic üretmeden reddeder. Dört entegrasyon testi derlenmiş host’u gerçek süreç olarak çalıştırır. Bu başlangıç yürütümünü ve bağımlılık sınırını kanıtlar; görev API/durum makinesi, günlük, HTTP servisi, yaşam döngüsü CLI’ı veya ilk tarayıcı gösterimini kanıtlamaz. G08 tipli görev sözleşmelerini, G14/G15 servis/CLI davranışını ekler. Başlangıç paket sürümü mevcut API/NeedSpec/model-set şema sürümlerini değiştirmez.

Depo, kabul edilmiş aşama kapısının istediği implementation / rust-v1 profiline, runtime_implemented true ile geçer. Bu alan uygulama aşaması kaynağını belirtir; tamamlanmış görev runtime’ı veya kabul edilmiş ürün anlamına gelmez. Mevcut ortak kaynak/ortam/denetleyici/workflow politikası değişmez. Belge checkout’u, ilan ettiği rolü değiştirerek Rust ekleyemez. Taşınabilir Wasm paket seçimi yalnız eylema-contracts içerir; native host Wasm’a derlenmez.

Tam rust-toolchain.toml, Rust 1.99.0, minimal profil, rustfmt/clippy ve wasm32-unknown-unknown sabitler. Kök workspace edition 2024 ve resolver 3 kullanır. Değişmeyen birinci taraf kapı locked/offline Cargo metaverisi, formatlama, uyarıları reddeden tüm hedef/özellik clippy, native test/build ve seçilen taşınabilir kütüphanenin Wasm build’ini çalıştırır. Cargo bağımlılıkları birinci taraf workspace yerel kalmalıdır. Host/tarayıcı/SDK/sürücü sınırları açık kalır.

```sh
env -u RUST_LOG .venv/bin/python tools/check_rust.py
cargo run --locked --offline --package eylema -- --version
cargo run --locked --offline --package eylema -- --help
```

Bu komutlar resmî kurulmuş rustup/Cargo executable’larının PATH üzerinde olmasını gerektirir; G04 shell başlangıç dosyalarını bilinçli olarak değiştirmedi. Gerekirse standart cargo/bin konumunu açık çağrı ortamına ekle. Kalıtılan RUST_LOG’u temizlemek yalnız ilgisiz Codex host günlük değişkenini kaldırır; kabul edilmiş ortam kapısı gevşetilmez. Altı AGENTS araç komutunu da çalıştır. Kaynak profili artık implementation olduğu için hosted ARM64 macos-26 CI gerçek Rust kapılarını yürütür; hosted yürütüm bu yerel M4 kabulünden ayrıdır.

Python 3.14.8, Rust 1.99.0 native ARM64, macOS 27.0.1 M4 Pro/24 GiB üzerinde yerelde geçti: formatlama/clippy, dört gerçek süreç testi, native workspace build, kendi bağımlılık envanterimiz ve seçilen contracts kütüphanesinin Wasm derlemesi. Not run: Wasm/tarayıcı yürütümü, görev/servis/gözetim/günlük davranışı, GPU, model, eğitim, performans veya sürüm kabulü. Güncel PR/main CI ve tam head üzerinde bağımsız inceleme ayrı teslim kapılarıdır; yerel kaynak derleme bunların yerine geçemez. [Görev kaydı](../../tools/work-plans/g07.json) kapsamı, tabanı, kabul edilen bağımlılıkları ve sonlu bütçeleri bağlar.
