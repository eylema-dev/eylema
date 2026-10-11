# G08 — Ortak tipli görev sözleşmeleri

İngilizce kanoniktir. [English](../en/typed-contracts.md). Runtime sözleşmesi **0.1.0**; paket derleme kimliği ve model-set şema sürümleri bağımsızdır.

Sahip olduğumuz [portable kütüphane](../../crates/eylema-contracts/src/lib.rs) artık doğrulanmış görev, eylem, öneri, execution anlık görüntüsü, etki, olay, yetki gereksinimi, bileşik gözetim, bütçe ve hata değerlerini tanımlar. Düzenlenebilir taslaklar, hata dönebilen kurucularla alanları özel doğrulanmış değerlere dönüşür. Değiştirilemez alan erişimi kurucuyu atlayamaz. Native host’un [tüketici testi](../../apps/eylema/tests/contracts.rs), [sözleşme testleriyle](../../crates/eylema-contracts/tests/contract_validation.rs) aynı genel tipleri ve ret semantiğini kullanır. Host komutu hâlâ yalnızca başlangıç help/version davranışını sunar.

Tam runtime sürümü 0.1.0 kabul edilir; daha eski, daha yeni minor/patch ve major sürümler, incelenmiş uyumluluk değişikliğine kadar reddedilir. Kimlikler ayrı Rust tipleri ve sınırlı opak ASCII metinleridir; yol ayırıcıları, boşluk veya joker içermezler. Üretilmiş küresel benzersiz kimlik ya da dosya sistemi yolu değildirler. Pozitif revizyonlar ve Unix milisaniye zamanları beyan edilen değerler olarak kalır; kurucu saate bakmaz. İnsan mesajları bayt sınırları içinde İngilizce/Türkçe UTF-8 destekler. Dilden bağımsız on üç API hata kodu İngilizce kimliklerini korur; yeniden deneme gecikmesi açıkça yeniden denenebilir hata gerektirir.

Yapısal sınırlar kimlik başına 96 bayt, insan metni başına 4096 bayt, sınırlı referans listesi başına 64 referans, öneri başına 1024 eylem ve beyan edilen payload için en çok 1 MiB’dır. Bunlar protokol girdi sınırlarıdır; ölçülmüş performans veya cihaz kapasitesi garantileri değildir. Bütçeler pozitif sonlu adım/olay/payload/eşzamanlılık değerleri ve açık deadline gerektirir. Öneriler görev/durum/politika/plan kimliklerini, benzersiz eylemleri, tam kaynak alt kümelerini ve istenen yürütüm hedeflerini bağlar; eylem beyanları üst görevin bütçesini veya toplam payload sınırını aşamaz. Okuma dışı görev eylemleri durum önkoşulu gerektirir. Backend çiftleri native CPU/GPU yok, native GPU/Metal, browser Wasm CPU/GPU yok ve browser WebGPU/WebGPU’ya izin verir; bu beyanlar kullanılabilirlik veya yürütüm kanıtlamaz.

Yetki değerleri gereken izinleri, kaynak kapsamını, principal referansını ve politika revizyonunu tanımlar. Kontrol noktaları, gözlem, müdahale ve yönetişim referansları bağımsız biçimde bileşir. Principal adı, capability, politika referansı veya doğrulanmış değer izin vermez. Kimlik doğrulama, politika çözümleme, delegasyon, sona erme/iptal ve kabul sonraki Goal’lerde uygulanıp doğrulanmalıdır. Burada onay kararı veya kriptografik payload güveni uygulanmaz.

Execution anlık görüntüleri tam öneri/görev/planı bağlar ve önerinin durum revizyonundan önce olamaz. Başarı, eksiksiz bilinen etki kayıtları ve sonuç referansları gerektirir. İptal isteği alındısı aktif kalır; etkin iptal duruş ve kısmi sonuç referansları gerektirir. Bilinmeyen etkiler başarılı, başarısız veya iptal edilmiş bilinen sonuçlara yükseltilemez; terminal belirsizlik OUTCOME_UNKNOWN gerektirir. Referanslar beyandır, doğrulanmış kanıt değildir. Sonraki yürütücü gözlemleri çözümler, etkileri yetkilendirir ve geçişleri yönetir; bu kurucular hiçbir şeyi yürütmez, iptal etmez veya geri almaz. Olaylar gerektiğinde execution kimliği, sınırlı farklı kanıt referansları ve açık revizyon/zaman gerektirir; küresel sıralama ve tekrar teslim ayıklaması sonraki runtime sorumluluklarıdır.

Kabul edilen native ARM64 ortamında Rust 1.99.0 ile yeniden üret:

```sh
PATH="$HOME/.cargo/bin:$PATH" env -u RUST_LOG .venv/bin/python tools/check_rust.py
cargo test --locked --offline --package eylema-contracts
cargo test --locked --offline --package eylema --test contracts
```

Mevcut kapı native biçimlendirme, uyarıları reddeden clippy, test ve derlemeyi çalıştırır; ardından yalnızca seçilen sözleşme kütüphanesini wasm32-unknown-unknown için derler. Wasm derleme, browser/Wasm yürütümünden ayrıdır. Güncel kanıt ve tam revizyonlar teslim PR’ında yer alır. G09 sınırlı wire kodlama/kanonik hash/platform kimliğini; G10 kabul/yürütümü; G11 gerçek gözetimi ekler. Servis, browser, journal, GPU, modeller, eğitim, performans ve tam ürün kabulü sonraki kapsamdır. Tüm Cargo paketleri birinci taraftır ve yayımlanmamıştır; bağımlılık, ayar veya lisans seçimi eklenmez.
