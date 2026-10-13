# G87 bellek sözleşmeleri ve sınırlı kaynak bağlamı

Taşınabilir `eylema_contracts::memory` sözleşmeleri ve native `eylema_host::memory` adaptörü kabul edilmiş [G63 proje veri kapısı](project-data.md) üstünde ilk bellek dilimini sağlar. [Bellek tasarımı](https://github.com/eylema-dev/devguide/blob/main/docs/tr/memory-layer.md) working, episodic, semantic, procedural, knowledge_graph ve analytical kullanımı ayırır. Bunlar isteğe bağlı platform yeteneğidir; altı yeni zorunlu model değildir. Adaptör **açık kaynak referanslarını** çözer; semantik benzerlik, graph dolaşımı veya analitik/SQL sorgusu G70/G88/G89/G90’dadır.

Değişmez profil kimlik, pozitif sürüm, giriş şeması, desteklenen bellek türü/router rolü ve backend niyeti bağlar. Kaynak bildirimi gerçek DataRef, tür/profil/şema, gözlem/valid-from/süre sonu, host beyanlı observed/reviewed/estimated/predicted durumu, isteğe bağlı bildirilen güven ve embedding uzayı kimliğini bağlar. Semantik kaynak space/model/ön işleme/sürüm/boyutu eşitlikle bağlar. Aynı boyut farklı model/ön işleme kimliğiyle eşleşmez. Alanlar inference, kalibrasyon, kaynak doğruluğu veya fiziksel embedding index doğrulamaz.

Yalnız mühürlü bellek registry owner ve gerçek G63 proje owner değişmez kaynak metaverisi kaydeder. Kaynak mevcut, put-frame özeti sıfırdan farklı, bildirilen saklama süresine uygun olmalı; eğitim adayı ağırlık yükü olmamalıdır. Kayıt registry’yi **o canlı proje deposu kimliğine** bağlar. Aynı proje/öğe kimliği, bayt ve digest taşıyan başka depo onun yerine geçemez; veri deposu yeniden açılınca geçici registry yeni kabul edilmiş host yetkisiyle tekrar kurulup kaydedilir. Bu host beyanıdır; gerçek insan kimliği veya otomatik gerçek doğrulayıcısı değildir. Yüksek güven doğrulama durumunu değiştiremez. Predicted/estimated öğe etiketlidir, verified-only isteği reddeder.

## Sorgu ve aktarım

Kapalı on bir alanlı canonical sorgu proje/profil/sürüm, router rolü, yerel amaç, türler, açık kaynak kimlikleri, beklenen embedding uzayı, verified-only filtre, bayt ve isteğe bağlı token tavanı taşır. Bilinmeyen/eksik/ek alan, tür, rol veya amaç reddedilir. İzin ver talimatı ekleyerek alan oluşturulamaz. Rol router seçicisidir; **çağıranın kimliği veya izni kanıtı değildir**. Gerçek DataSession amaç/kapsamı ve bütün G63 kaynak soyu erişimi yönetir.

Adaptör özel kaynak durumunu göstermeden güncel veri iznini denetler; değişmez kaynak/profil/şema/uzayı eşler, güvenilen zamanı denetler ve G63 üzerinden uygun UTF8 içerik okur. Tam canonical EYL bağlamını hazırlar, kaynak bilgisi dahil gerçek serileştirilmiş baytı sayar, dönmeden önce güncel kaynak kullanımı ve kaynağa özel TTL’yi tekrar denetler. Zarf istenen rol/amaç, proje/profil sürümü, oluşturma başlangıç zamanı ve kaynak başına kimlik, put özeti, tür/şema/profil, zaman/durum/bildirilen güven/embedding kimliği, gerçek ebeveyn referansı ve yaprak politika sürümünü korur. Etiket grant değildir; yaprak sürüm/bayrağı güncel üst-kaynak kapısının yerini alamaz.

Önbellekli sorgu yalnız referans/parametre ve registry kimliği taşır; içerik veya yetki makbuzu saklamaz. Her tekrar güncel izin, kaynak süre sonu/geri çekme ve semantik bağı denetler. Kaynak geri çekme, amaç hakkı değişimi veya kaynak TTL aşımı yeni bağlamı engeller. Başka registry cache’i reddedilir. Önceden dönmüş bayt geri alınamaz; ileride araç/provider gerçek aktarımında yeniden yetkilendirmelidir. Sonuç `Debug` yalnız sayı bildirir, bağlam yükünü göstermez. Model prompt’u veya özel muhakeme cache/bellek journal’ına otomatik toplanmaz.

Profil owned_graph/owned_analytical/sqlite_reference/duckdb_reference niyeti bildirebilir; prototip bu seçimlere `BackendUnavailable` döner. Yalnız owned_references uygulanmıştır. Rust binding/enum SQLite/DuckDB kurmaz veya motor istisnası onaylamaz. Sorgu model seçimi sınırı sessizce geçemez.

Token sınırlı istek gerçek kabul edilmiş tokenizer sayımı gelene kadar `TokenCounterUnavailable` döner. `tokens: None` bilinmiyor demektir; sıfır veya boşluk tahmini değildir. Bayt sınırı uygulanır. Boş açık-referans isteği profil/proje doğrulamasından sonra boş kaynak kümesi döndürür; özel öğeye erişim veya izin grant’i oluşturmaz.

| Kabul sınırı | Limit |
|---|---|
| Profil / kayıtlı kaynak | 1–128benzersiz profil;0–128benzersiz kaynakID |
| Sorgu türü / router rolü | En çok6ayrı tür;profil1–9ayrı rol |
| Açık istenen kaynak | 0–32ayrıID |
| Serileştirilmiş bağlam | Kaynak bilgisi dahil1–65536bayt |
| Hazırlanan kaynak gövdeleri | En çok32G63gövdesi,her biri64KiB;serileştirme öncesi2MiB |
| Embedding boyutu / bildirilen güven | 1–4096 / isteğe bağlı0–10000baz puan |

Bunlar nesne/işlem yapısal sınırıdır; global allocator tepe değeri, latency garantisi veya ölçülmüş token/model throughput değildir. Çağıranın tuttuğu nesne/cache sayısı host bütçesi sorumluluğudur. G63 saat gözlemi senkronize olup journal kapasitesi tüketebilir. Kaynak bildirimi backing data süre sonunu uzatamaz; gelecekte geçerli kaynak güvenilen zamanına kadar kullanılamaz. Registry metaverisi geçicidir, tekrar kurma owner yetkisi gerektirir; yeni kalıcı bellek/index biçimi yoktur. Mevcut görev/veri/wire/journal biçimleri değişmez.

## Yürütülmüş kanıt

ARM64 macOS/Rust1.99.0’da dört yeni taşınabilir ve on bir yeni native adaptör testi geçti. Altı tür etiketi, kapalı sorgu roundtrip/yetki alanı reddi, eksik kaynak/zaman/uzay/geçersiz güven, tekrar/sınır, eşit-boyut semantik uyuşmazlık, gerçek amaç kontrollü bağlam, özel durum reddi, rol/amaç/proje/yabancı owner/oturum/profil reddi, tahmin, kaynak geri çekme/politika/TTL/cache kimliği, oluşturma sırasında ilerleyen saat, aynı başka backing depo reddi, geçersiz UTF8, gerçek serileştirme yükü, opak debug,128profil/kaynak sınırı, boş sonuç ve desteksiz motor/token bütçesini sınar.

Test-first koşusunda eksik modüller, ardından üç somut güvenlik regresyonu—başka depo ikamesi, özel durum sorgulama ve oluşturma sırasında TTL—hata verdi; düzeltmeler geçti. Native adaptör fixture’ları kendi bellek içi depomuz ve kontrollü saattir; **yeni fiziksel dosya, insan veya model kabulü değildir**. Tam native suite G63’ün ayrı kapsamlı gerçek dosya testini de korur.204Rust/47Python,fmt,warnings-deniedclippy,nativebuild ve seçili taşınabilir kütüphane Wasm derlemesi son head’de geçmelidir; PR kanıtı ayrı inceleme/güncelCI/main kabulüne bağlanır. Mevcut Metal donanım testi normal suite’te ignored’dır, bu kart için gerekmez.

Bu dilimde not_run: yeni bellek modülü Wasm yürütmesi, gerçek vector/graph/analitik/SQL motoru, tokenizer/inference/eğitim, insan kimliği, tam harness görevi ve performans kabulü. Uygun içerik, doğru kaynak/sır olmayan kimlik, doğru host beyanı ve cooperative depolama/saat/hash ön koşuldur. Üçüncü taraf crate, gizli motor, yeni provider/model/eğitim/G40 denemesi veya harcama yoktur. Geri dönüş G63 journal’larını koruyarak incelenmiş PR ile sınırlı API/registry’yi kaldırır; türetilen bağlam migration veya yeni yetki belgesi değildir.
