# G46 döngü, rol ve model bağlama sözleşmeleri

Taşınabilir `eylema_contracts::platform` modülü tanımlı harness metaverisini doğrular. Platform metaverisi sürümü 0.1.0'dır; mevcut görev sözleşmesi 0.1.0 ve EYL/EYLJ v1 değişmez. [Güncel aksiyon planı](https://github.com/eylema-dev/devguide/blob/main/docs/tr/platform-action-plan.md) ve [dokuz rol](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/architecture.md#rust-harness-model-rolleri-ve-alana-göre-seçim) açıklamaları bağlayıcıdır.

`RoleKind`, perception, reasoning, planning, decision, policy, world_model, specialist, critic ve representation rollerini ayırır. Döngü en az bir rol ister; dokuzunun tamamı gerekmez. Bir modelin birden çok profili olabilir; bir uyumlu profili birkaç bağ kullanabilir. STT/TTS/görüntü/metin alt profilleri platform planında ayrı tanımlanır; bu modül bunların inference yürütmesini sağlamaz.

`RoleInterface`, pozitif anlam sürümünü, girdi/çıktı şema ve birimlerini, ön işlemeyi, eylem anlamını ve durum sıfırlamayı kesin profil kimliğiyle bağlar. Model profili model/profil kimliklerini, tanımlı 32 bayt artifact kimliğini, pozitif artifact sürümünü ve hedef yürütme yerini içerir. Sıfır digest yer tutucu olarak reddedilir. Burada dosya/hash/şema/model okunmaz veya doğrulanmaz; profil kimliği eşitliği fiziksel davranışı, artifact bütünlüğünü, hakları veya desteği kanıtlamaz.

`ModelRegistry` boş, yinelenen veya aşırı profil kümelerini reddeder. `LoopDefinition` kesin profil arayüzlerini çözer ve seçilen profilleri bağ sırasıyla değişmez kopyalar olarak tutar. Registry değişimi kurulmuş tanımı değiştiremez. Kenarlar var olan uçları ve kaynağın çıktısıyla hedefin girdisinin şema/birim uyumunu gerektirir. Geri besleme döngülerine tanımlı sonlu pozitif adım/iterasyon sınırlarıyla izin verilir; yürütüm sırasında uygulama gelecekteki denetleyicinin sorumluluğudur.

| Yapısal üst sınır | Limit |
|---|---|
| Model profilleri | 1–128 |
| Rol bağları | 1–32, benzersiz kimlikler |
| Kenarlar | 0–128, benzersiz yönlü çiftler |
| Adımlar | 1–4096 |
| İterasyonlar | 1–1024 |

Bunlar girdi sınırlarıdır; ölçülmüş throughput/kapasite değildir. Kimlikler mevcut sınırlı opaque ASCII ResourceId sözleşmesini sarar. `LocalOnly`, `ExternalConfigured` yürütme yerini reddeder. `ExplicitExternal` yalnız metaveri kabul eder; erişim, dış açıklama, ücretli çağrı, yürütme veya yedek yol yetkisi vermez. ProjectId mantıksal ilişkidir; kimlik doğrulama veya dosya sistemi yalıtımı değildir. Model çıktıları öneri kalır; gerçek araç/izin/veri sınırları G54/G63/G73 işidir.

## Çalıştırılmış kanıt

Bu PR head'inde ARM64 macOS, Rust 1.99.0 ve Python 3.15.0 ile yeni integration testleri aynı sözleşmelerden iki ayrı döngü tanımını, her isteğe bağlı rolü, tüm arayüz uyumsuzluklarını, registry/rol/kenar tekrar ve sınırlarını, eksik referansları, sıfır artifact kimliğini, kenar şema/birim uyumsuzluğunu, yerel döngü reddini ve değişimde snapshot kimliğini sınar. Test önce koşusu eksik platform modülünde başarısız oldu; uygulama sonrası testler geçti.

Geçti: 5 yeni sözleşme integration testi, 176 tam workspace Rust testi, sabitlenmiş fmt/clippy/build ve seçilmiş taşınabilir kütüphane Wasm derlemesi; 47 Python testi, depo/PR/diff kontrolleri ve yenilenmiş EN/TR fingerprint. Yalnız kontrol sürecinin PATH'i kurulu toolchain'i seçer; incelenen runner bu override'ı reddettiği için kalıtılmış RUST_LOG kaldırılır. CI kendi incelenmiş ortamını kullanır ve kesin SHA'ları kaydeder. Fingerprint anlam eşliği kanıtı değildir; son head üzerinde ayrı inceleme gerekir.

Çalıştırılmadı: yeni modülün Wasm yürütümü, model inference, eğitim, GPU performansı, gerçek insan kaydı ve tam harness işleri. Constructor testi bunları kanıtlamaz. Harici crate, provider çağrısı, model indirme, eğitim kaydı, üretim motoru veya G40 denemesi eklenmez.

Geri dönüş, modül/export ve kapsamlı notları incelenmiş PR ile kaldırır. Bu kart serialization, kalıcı kayıt veya runtime etkisi eklemediği için mevcut görev/wire verisi taşıma gerektirmez.
