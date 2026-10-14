# G10 — Deterministik gömülü yürütücü

[English](../en/deterministic-runtime.md). Çalışma zamanı sözleşmesi0.1.0, yayımlanmamış paket0.1.0. [Plan](../../tools/work-plans/g10.json), [issue12](https://github.com/eyleme-dev/eyleme/issues/12), [codec/platform](codec-platform.md).

Birinci taraf güvenli [runtime](../../crates/eylema-runtime/src/lib.rs), süreç içi görev kabulü, FIFO kuyruk, sınırlı tek adım yürütümü, anlık görüntüler, gözlemler ve olaylar sağlar. Üretim bağımlılığı yalnız eylema-contracts’tır; birinci taraf platform kütüphanesi gerçek native tüketici/test için geliştirme bağımlılığıdır. Sunucu, üretim arka plan worker’ı, rastgele skill yükleme, dış etki, journal veya tarayıcı sağlamaz. Host executable help/version davranışı bootstrap olarak kalır; ayrı native örnek bu gerçek kütüphaneyi çalıştırır.

## Host yetkisi ve desteklenen etkiler

Runtime::new, Runtime ile özel, serialize edilmeyen HostKey döndürür. Yalnız eşleşen anahtar sınırlı Grant’i (güvenilen gömen host tarafından doğrulanan principal, kaynak kapsamı, izinler, politika revizyonu) kaydedip opak Session üretebilir. Güvenli Arc kimliği handle’ları tek runtime’a bağlar; diğer runtime’ın anahtarı/session’ı başarısız olur. Rust istek principal/yetki referansı dizeleri session üretemez. Bu güvenilen gömülü capability sınırı, ağ kimlik doğrulaması veya insan doğrulama yöntemi değildir; servis yerel session sınırı G14’e aittir.

Kabul ve her adım session’ı, kapsamlı ExecuteAction iznini, beyan edilen principal/politikayı ve istenen tüm izinleri yeniden kontrol eder. readTaskValue, istek bu gereksinimi atlamış olsa bile ayrıca Read ister. Gerekli gözlem ObserveEvents ister. Zorunlu checkpoint APPROVAL_REQUIRED; gerekli müdahale/governance, G11 kanıt/davranış sağlayana kadar CAPABILITY_UNAVAILABLE döndürür. Referans tek başına onay kabul edilmez. Desteklenen yürütüm task:state capability ile native macOS CPU’dur. Bilinmeyen operation/capability, GPU/tarayıcı hedefi ve tutarsız etki türü/argüman, etkiden önce başarısız olur.

| Operation | Argüman ve etki |
|---|---|
| readTaskValue | Boş payload, Read sınıfı; sahipli değeri gözler, mutasyon için NotApplied kaydeder |
| addTaskValue | Kanonik EYL unsigned u64, RecordMutation sınıfı; sahipli görev değerine kontrollü toplama |

Görev oluşturma value0 ile başlar. Başarılı her toplama canlı TaskSpec revizyonunu bir artırır; taşma o adımın mutasyonundan önce başarısız olur, önceki doğrulanmış etkiler korunur. Tek aktif execution, task lease tutar. Değişmeyen proposal/onay tabanı kabul edilen snapshot’tır; iç beklenen canlı revizyon yalnız kendi gözlenen mutasyonlarıyla ilerler. Yeni eski sürüm gönderimleri REVISION_CONFLICT ile reddedilir. Her adımın gözlenen revizyon/lease kontrolü sessiz yeniden bağlamayı önler; mevcut API dış task writer sağlamaz. ExecutionRecord state_revision plan tabanını; olay/task görüntüleri gözlenen canlı revizyonu gösterir. Başarısız plan lease’i bırakır, kısmi değer/revizyon/kanıt korunur.

## Kabul, replay ve kaynak sınırları

G09 kanonik onay byte’ları etki alanlarını bağlar, taşıma context’lerini çıkarır. Idempotency anahtarı principal, operation, kaynak ID’leri ve idempotency key’den; hash host’un standart SHA-256 ile kanonik beyanı hash’lemesinden oluşur. Aynı replay, deadline sonrasında da yeni kabulden önce aynı queued/completed execution’ı döndürür; yeni etki üretmez. Değişen içerik veya kısmen yeniden kullanılan batch conflict olur. Proposal ID + plan revizyonu da yeni idempotency key ile farklı kanonik byte’lara yeniden bağlanamaz. Yeni plan/durum sürümü uygun yeni bağlar ister.

Kayıt/replay referansları sınırlı kapasite içinde tüm süreç ömrü boyunca tutulur; eviction, otomatik retry, restart retention, durable acknowledgement veya dış etki exactly-once iddiası yoktur. Kapasite dolunca yeni kabul BUDGET_EXCEEDED olur; belirsiz süreç kaybından sonra sessizce yeni operation oluşturulmaz. Kalıcılık/recovery G12/G13’e aittir.

Varsayılanlar yapısal sınırdır, ölçülmüş performans değildir:64 task,128 execution,16 session,32 queued execution,4096 olay slot’u,4096 replay key ve16MiB resident wire hesabı. Yapılandırılmış adetler1..65536; resident hesap1byte..1GiB olmalıdır. Task/serialize proposal byte’ları ayrı hesaplanır; metadata/allocator maliyeti adetlerle sınırlıdır ama bu wire bütçesinde ölçülmez. Runtime kaynağının Wasm yürütümü iddia edilmez; portable seçim contracts/platform olarak kalır.

Task adım/olay bütçeleri birikimlidir. Kabul tüm planlı adımları ve N+3 olay slot’unu (accepted, running, N gözlem, terminal); task oluşturma bir olayı tahsil eder. Başarısızlıkta kullanılmayan rezervler temkinli biçimde korunur, otomatik iade edilmez. Gerçek yapılan adımlar ayrı raporlanır. Tek worker concurrency1 sağlar; task lease aynı task’a örtüşen planları da önler. Kabul reddi task/kuyruk/ledger/olay durumunu commit etmez; başarısız olabilen host saat/hash/ID çağrıları yapılmış olabilir. ID ve terminal kapasite etkiden önce ayrılır; tekrarlanan kaynak ID’si başarısız olur. Deadline kabulden önce, metadata rezervasyonundan sonra tekrar ve her adımdan önce kontrol edilir. Saat hatası queued işi değiştirmez. Bu işbirlikçi kontroller hard realtime kesinti veya fiziksel sonlanma kanıtı değildir. Deadline hatasında etkili duruş kanıtı bulunmaz.

## Gözlemler ve gerçek kanıt

Her olayın evidence referansı, event/task/execution ID’si, gözlenen timestamp ve önce/sonra değerini taşıyan sahipli Observation’a çözülür. Bunlar senkron sahipli belleğin gerçek gözlemleridir, dış etki kanıtı değildir. Olay/snapshot okuması kapsamlı Read/ObserveEvents uygular. Olaylar tutulan task akışında sıralıdır; sınırlı limit1..256 ve bilinmeyen cursor için CURSOR_EXPIRED açıktır. Completed/failed snapshot G08 constructor’larını kullanır; kabul Queued’dur, başarı değildir. Etkiler ayrı Applied/NotApplied’dir; terminal sonuçlar correlation ve kısmi gözlemleri korur.

Yerel macOS27.0.1 ARM64/M4Pro24GiB, Rust1.99.0, Python3.14.8:

```sh
PATH="$HOME/.cargo/bin:$PATH" env -u RUST_LOG .venv/bin/python tools/check_rust.py
PATH="$HOME/.cargo/bin:$PATH" env -u RUST_LOG cargo run --package eylema-runtime --example executor-proof --locked --offline
```

Passed: değişmeyen Rust koruma/envanter/fmt/clippy/native45test/build ve seçilen iki portable kütüphanenin Wasm derlemesi. On dört yeni executor regresyon testi başarı, taşma/kısmi etki, runtime’lar arası/beyan edilen yetki, reddedilen izin/checkpoint/müdahale/governance/gözlem, replay/conflict, kabul/adım/host rezervasyonu sonrası deadline, FIFO/lease/eski durum/politika/hedef/capability, kaynak bütçeleri, geçersiz config/session/ID/cursor, saat hatası ve değişmeyen proposal-revizyon kimliğini denetler. Deterministik saat/ID fixture’ı geliştirme testidir, işletim sistemi saati/entropy yürütüm kanıtı değildir; fixture hash’i gerçek native CommonCrypto kullanır.

[Gerçek native tüketici](../../crates/eylema-runtime/examples/executor-proof.rs), SystemTime, CommonCrypto/Security hizmetleri, host tarafından kayıtlı kapsamlı session, gerçek queued/running/succeeded durumları, iki adım0→2→5, altı gözlenen olay ve aynı queued/completed replay’i kullanır. Bu native yürütüm belge CI’ından ayrıdır. Geliştirme derleme/stil hataları korumalar zayıflatılmadan düzeltildi; güncel başarılı kontroller teslim revizyonuna bağlıdır. Gerçek insan onayı/revocation/etkili iptal, journal/restart/HTTP/CLI/tarayıcı kabulü, GPU/model/eğitim ve performans: not_run, sonraki Goal’ler. Yayın veya lisans seçimi sağlanmaz.

G10 metnindeki eksik onay/geri alma/iptal ifadeleri kabul edilmiş G10 sürümünü anlatır. [G11 insan denetimi](human-oversight.md), bu native çekirdeği tutulan kararlar ve doğru işbirlikçi iptalle genişletir; kendi kanıtları ve kalan kimlik doğrulama/host/tarayıcı sınırları ayrıdır.

G54 aynı iki işleyici üstüne [tipli araç seçimi ve kaynak kontrollü yürütme](typed-tools.md) ekler. Mühürlü cephe biriken değerlerde kaynak kısıtlarını korur; kalıcı tipli kaynak soyu olmayan kurtarılmış host’u reddeder. Önceki yürütücü arayüzleri bu cephe için gizlilik aşma yolu değildir.
