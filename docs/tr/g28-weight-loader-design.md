# G28 sınırlı birinci taraf ağırlık yükleyicisi tasarımı

[İngilizce kanonik kaynak](../en/g28-weight-loader-design.md). İnsan incelemesi için yazılı tasarım; **üretim uygulaması veya kabul değildir**. Bakımcı 14 Ekim 2026'da kapsam/tasarım hazırlığını onayladı; yazılı tasarım ve G28’e özel profil bağı artık onaylıdır; somut plan incelemesi bekliyor. Kaynak tabanı `6605080fcae225e099775f7856349c0aac6aa011`, rehber tabanı `affbcd456fc467b10155eb3b71b4860042220b30`; core issue [G28 / #25](https://github.com/eylema-dev/eylema/issues/25).

## Amaç ve katkı sınırı

Gerçek artifact baytlarını kendi Rust kodumuzla yükleyip doğrulayarak sonraki yerel model motoruna değişmez, kimliği belirli bir ağırlık snapshot'ı sağlamak. G09 platform/codec ve G27 manifest kabul edilmiş bağımlılıklar olarak kalır. G28-P1/A1/CP1, özgün zorunlu bağımlılıklar ve 120 aktif dakika / 3 hedefli onarım / 15 ayrı inceleme dakikası korunur; bütün hazırlık aynı bütçeden düşer. Model/ağırlık indirme, provider/eğitim denemesi, yeni harcama veya üçüncü taraf Cargo paketi yok. G40 ve diğer bütün kartlar korunur.

Mimar/yazar/doğrulayıcı ayarı kayıtlı `gpt-6.1-sol/high`; son tamamlanan yanıt metaverisi bu yapılandırmayı doğrular, gizli sunucu kimliğini doğrulamaz. Gerçekten ayrı, uygun profilli incelemeci ve son kaynak sürümüne bağlı kanıt gerekir. Mevcut `sol61-high-contracts-2026-10-14` kendi sözleşme/validator çalışmalarını, geliştirme testlerini ve iki dilli belgeleri kapsar; 13 Kasım 2026'ya kadar geçerlidir. Yapısal artifact doğrulaması bu tanımlı alana uyar. Yeni artımlı native binding G27 kanıtında açıkça listelenmemiştir: **gerçek bakımcı bu yazılı tasarımı inceleyerek yalnız G28 bağını doğruladı; somut plan onayı ve çalıştırılmış kalite kontrolleri hâlâ gereklidir**. Salt okunur değerlendirme yeni sayısal/kriptografik kernel, performans veya başka Goal'i yeterli saymaz. G23'e özel sayısal profil G28'i kabul edemez. Bu tasarım mevcut yeterlilik kaydını genişletmez.

## Kapsam ve mimari

Onaylı hazırlık yalnız `crates/eylema-model/` için kök `Cargo.toml`/`Cargo.lock` kaydını ve platform crate'inde `crates/eylema-platform/src/lib.rs`, yeni `src/sha256_stream.rs` ile `tests/sha256_stream.rs` dosyalarını ekler. Mevcut eşlenmiş belge/fingerprint kapsamı korunur. Model crate'i `forbid(unsafe_code)` kullanan güvenli Rust'tır; yalnız kendi contracts/platform bağımlılıkları bulunur. Önerilen modüller sınırlı header ayrıştırma, beklenen layout, snapshot yükleme ve hata kategorilerini ayırır; build script, harici parser/çıkarım motoru veya kopyalanmış uygulama bulunmaz.

Girdiler önceden doğrulanmış değişmez G27 `ModelManifest`, ayrıca tanımlanmış tam `WeightLayout`, açık `LoadBudget` ve çağıranın sahip olduğu `Read` uygulamasıdır. Yükleyici yol açmaz, link izlemez, dosya indirmez veya proje erişimi vermez. Çağıran izinli girdi ve bloklama/süre sınırı olan okuyucu sağlamalıdır; genel senkron `Read` kendi başına zamanla iptal garantisi veremez. Yükleme bütçesi mantıksal bayt/iş sınırıdır; host yetkisi veya fiziksel kapasite kanıtı değildir.

Layout, kanonik manifest digest'ini, tam yapılandırılmış ağırlık bileşenini, desteklenen saklama hassasiyetini ve sıralı, benzersiz tam tensor haritasını bağlar. Constructor'lar haritayı doğrulayıp özel olarak sahiplenir. Çağıranın verdiği haritanın gerçek model mimarisini bütünüyle kapsadığını kanıtlamaz. Dönen sonuç beyan edilen layout ile gerçek bayt kimliğini bağlar; `check_assignment` veya desteklenmeyen yerel model durumunu değiştirmez. Eksik kaynak, haklar, beklenen harita kökeni veya mimari uyumu ayrı model kabul kapısıdır.

Snapshot yüklemesi tek sahiplenilmiş artifact buffer'ı tutar. Önce küçük header okunup doğrulanır; büyük artifact ayrılmadan bozuk metaveri reddedilir. Artifact için bir kez, hata döndürebilen bellek rezervasyonu yapılır; tam prefix/header eklenir, payload sınırlı scratch belleğine en fazla 64 KiB parçalarla okunur. Her prefix/header/payload baytı native stream ile özgün dosya sırasında hash'lenir. Erken EOF, fazla veri, okuyucu hatası, fazla deneme veya digest uyuşmazlığında kısmi sonuç dönmez. Tam doğrulama sonrası sahiplenilmiş buffer'dan salt okunur ödünç tensor bayt dilimleri sağlanır. Çağıranın özgün dosyasını değiştirmesi/kapatması dönen baytları değiştiremez. Bu süreç içi snapshot'tır; kalıcı depolama veya garantili silme değildir.

## Sabit profil sınırları

| Sınır | İlk sözleşme |
|---|---|
| Header dahil bütün artifact | En fazla 2 GiB; manifest'teki tam pozitif bileşen boyutu |
| JSON header | En fazla 1 MiB; prefix boyutu bellek ayrılmadan denetlenir |
| Tensor sayısı / rank | 1..1024 / 1..4; her boyut pozitif |
| Saklanan tensor hassasiyeti | Tek tip `BF16` veya `F32`, row-major, little-endian; manifest `safetensors.v1`, `bfloat16` veya `float32`, `none` quantization eşleşir |
| Tensor adı | 1..128 ASCII baytı, mevcut düz ad karakter alt kümesi; decode edilmiş adlar benzersiz |
| Metaveri | İsteğe bağlı `__metadata__` string haritası; en fazla 128 çift, key <=128 decode edilmiş bayt, value <=4096; yetki veya çıktı log'u olmaz |
| JSON derinliği / value node | En fazla 6 / 16384; iniş/ekleme öncesi sayılır |
| Okuyucu çağrısı / kesinti | Son EOF kontrolü dahil toplam en fazla 65568 çağrı; en fazla 16 interrupted tekrar |
| Resident içerik bütçesi | Artifact baytları ve sınırlı header/parser/layout/scratch için sabit 4 MiB pay; artifact rezervasyonu öncesi denetlenir |

Çağıranın resident bütçesi bu checked toplamı karşılamalı, artifact sınırı manifest bileşeninin tam boyutunu kapsamalıdır. Açıkça sahiplenilen büyüyebilir içerik buffer'ları kendi sınırlarında hata döndürebilen rezervasyon kullanır. Allocator/OS muhasebesi, çağıranın önceden sahip olduğu girdi, diğer modeller, genel OOM ve gözlenen RSS bu mantıksal hesabın dışındadır; tam fiziksel tepe veya başarılı 2 GiB tahsis sonucu çıkarılmaz. Korunan Qwen bileşeninin beyan edilen boyutu 1503300328 bayttır ve önerilen tavanın içindedir; gerçek artifact burada indirilmiş, yüklenmiş veya doğrulanmış değildir. G28 kabulünde 2 GiB bellek ayrılmaz.

Yasal fakat desteklenmeyen Safetensors özellikleri açıkça reddedilir: scalar/boş tensor, packed/sub-byte dtype, quantized layout ve sharded index. BF16/F32 payload opak kalır; float dönüşümü, NaN/Inf taraması, tensor aritmetiği, tokenizer veya model çalıştırma sağlanmaz. Sonraki dönüşüm/operatör kabulü sonlu sayısal değerleri ve hassasiyet semantiğini denetlemelidir. Fallback veya otomatik model terfisi yoktur.

## Header, tensor ve kimlik kontrolleri

Resmî biçim unsigned little-endian 8 bayt header uzunluğu, `{` ile başlayan UTF-8 JSON, tensor'a göre half-open data offset ve tamamen indekslenmiş ardışık payload tanımlar. Sabitlenmiş yayıncı [biçim tanımı](https://github.com/safetensors/safetensors/blob/e246a2560645b7525f5775669ed816eb57c5bcc8/README.md#format) okunmuştur; harici kütüphane kurulmuş veya çalıştırılmış değildir.

Kendi parser'ımız yalnız seçilen profilin gereken object, string, unsigned decimal integer ve array türlerini kabul eder. Sayılarda işaret, leading zero, fraction/exponent ve checked taşma reddedilir. JSON boşlukları ile string escape/UTF-8 açıkça işlenir; escaped ASCII alias dahil eşitlik decode edilmiş key üzerinden değerlendirilir. Bozuk escape/surrogate/UTF-8/control dizileri reddedilir. Son object sonrası header padding'i yalnız ASCII space'tir. Her object derinliğinde duplicate key reddedilir. Her tensor object'i tam `dtype`, `shape`, `data_offsets` içerir; bilinmeyen/eksik/duplicate alan reddedilir. Metaveri value'ları string olmalıdır; metaveri doğrulanıp atılır, talimat olarak yorumlanmaz veya tutulmaz.

Checked boyut çarpımı ile dtype genişliğinin çarpımı `end-start` olmalıdır. Offset'ler `0 <= start < end <= payload_length` şartını sağlar; sıralanan aralıklar tam `[0,payload_length)` olmalı, hole/overlap veya indekslenmemiş son ek bulunmamalıdır. Ad/dtype/shape beyan edilen haritayla tam eşleşir; eksik/fazla tensor yoktur. Yalnız bayt uzunluğu eşleşti diye hassasiyet değişemez. Bütün dosya uzunluğu ve SHA256 tam manifest bileşeniyle eşleşir. Digest kaynağı özgün publisher-advertised/measured beyanını korur; gerçek bayt karşılaştırması ayrıca kaydedilir ve yayıncı kimlik doğrulaması, hak yetkisi veya model yeterliliği değildir.

Önerilen public arayüz adları; imzalar somut planda sabitlenecek:

```text
StoredDtype::{BF16,F32}; TensorSpec; WeightLayout; LoadBudget;
LoadedWeights; LoadError;
WeightLayout::new; load; LoadedWeights::tensor_bytes;
Sha256Stream::new; Sha256Stream::update; Sha256Stream::finish;
```

Özel alanlar ve salt okunur ödünç accessor'lar denetlenmemiş değişikliği önler; `DerefMut`, değiştirilebilir tensor baytı veya parser durumu dışarı çıkmaz. Hata kategorileri bozuk profil/header/layout, boyut/iş/resident sınırı, eksik/fazla tensor, hash uyuşmazlığı, tahsis hatası, I/O ve unavailable/failed platform servisini içerir. Ham artifact/header/metaveri, özel yol veya çağıran verisi log'lanmaz.

## Native SHA256 yaşam döngüsü ve açık platform girdisi

Mevcut tek seferlik `sha256` ve 4 MiB wire sınırı değişmez. Yeni stream 1..2 GiB beyan edilen bayt sınırı, her biri <=1 MiB parça ve checked toplam sayaç kabul eder. Boş update sınırlı no-op'tur; boş digest referans testlerinde geçerlidir. Büyük/taşan/servis hatalı update stream'i zehirler; sonraki update/finalization reddedilir. Finalization stream'i tükettiğinden güvenli API'de tekrar kullanım mümkün değildir. Canlı native durum klonlanmaz veya host pointer tutulmaz.

Sağlayıcı macOS CommonCrypto olarak kalır. İncelenen macosx27.0 SDK `CC_LONG` için `uint32_t`, `CC_SHA256_CTX` için bu türden count[2], hash[8], wbuf[16] tanımlar; C uyumlu sahiplenilmiş context için beklenen size104/alignment4'tür. `CC_SHA256_Init/Update/Final` int döndürür ve1 olmalıdır. Yalnız platform crate'inde canlı hizalı context, checked parça dönüşümü ve ödünç girdi/32 bayt çıktı yaşam süresiyle dar, belgelenmiş unsafe çağrılar yapılır. Context alanlarında pointer yoktur; SDK uygulaması kopyalanmaz. FFI dışında context erişimi özel ve güvenli kalır. Desteklenmeyen native/Wasm target'lar `Unavailable` döndürür; mevcut seçili Wasm derlemesi çalıştırmadan ayrı kalır.

Birincil [Apple API belgesi](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/CC_SHA256.3cc.html) ve SDK `usr/include/CommonCrypto/CommonDigest.h` yalnız okunmuştur; header SHA256 `83a9705bbc8d44f27ee61801064ee99288496a038d623f152aa30db6819e9ca6`. Native çağrı, ABI çalışması ve hata davranışı bu tasarım aşamasında **not_run** durumundadır. Sonraki zorunlu kontroller SDK layout'unu ve bilinen digest/parça/yaşam döngüsü davranışını wrapper kabulünden önce bağımsız doğrular. Kendi kripto kernel'imiz, harici crate, host kabiliyetini açma veya mevcut tarayıcı hashing değişikliği yoktur.

Sonraki ABI kontrolü kurulu SDK'ya karşı tek geçici, yalnız geliştirmeye yönelik C layout assertion'ı (en fazla 30 saniye ve 64 KiB çıktı) ile Rust size/alignment kontrollerini kullanır. C üretim kodu dağıtmaz veya model/hash provider çağırmaz. Mevcut SDK/Clang açık geliştirme girdisidir; kurulum/build script eklenmez. Derleme/salt okunur SDK bildirimi tek başına native digest davranışını kanıtlamaz.

## Kabul, teslim ve kalan insan incelemeleri

| Ölçüt | Somut planda gereken kanıt |
|---|---|
| G28-P1 | Küçük sentetik BF16/F32 baytlarının başarılı yüklenmesi; tam layout/bütün dosya kimliği ve değişmez dönen snapshot; bozuk offset/dtype/shape/profil reddi |
| G28-A1 | Bağımsız dondurulmuş container baytları/digest ve referans tensor dilimleri; duplicate/escaped/unknown alan, hole/overlap, sıfır/taşan shape, eksik/fazla/değişmiş bayt, kısa/fazla/hatalı/interrupted girdi ve bütçe reddi |
| G28-CP1 | Gerçek uygun görev kapsamlı model/rol/bakımcı bağı, gerçekten ayrı son sürüm incelemesi, güncel yerel/CI kabulü ve mevcut koşullu merge yetkisi |
| Native bağımlılık | Bağımsız empty/abc/standard long SHA256 vektörleri, split pattern/block boundary, >4 MiB sentetik stream karşılaştırması, poison/limit/unsupported-target ve SDK ABI kontrolleri; eski wire sınırı regresyonları korunur |

Yalnız kendi sentetik fixture'larımız kullanılır; her biri <=8 MiB, test sürecinde aynı anda resident sentetik girdi/çıktı toplamı en fazla32 MiB ve sınırlı metaveridir. Python standart kütüphanesi referans bayt/digest'leri Rust kontrollerinden önce bağımsız dondurulur; Python geliştirme aracıdır, üretim parser/hash motoru değildir. Gerçek okuyucu hatası ve kaynak mutasyonu regresyonları gösterilmelidir; kaynak metni assert'i veya uydurulmuş RED yoktur. Büyük boyut/taşma örnekleri metaveri/sahte reader kullanıp büyük dosya yaratmadan tahsis öncesi başarısız olur. Tam çalıştırma/skip sayıları ve hatalar korunur. Küçük fixture gerçek Qwen mimari/layout, büyük dosya kapasitesi, throughput, model veya eğitim desteğini kanıtlamaz.

İnsan yazılı tasarımı onayladıktan sonra somut sohbet içi uygulama planı hazırlanır ve gereken incelemesi alınır. Özgün 120/3/15 bütçe hazırlık/değerlendirmeyi içerir; sıfırlanmaz. Ancak sonra uygulama, değişmeyen tam core kontrolleri ve fetched rehber doğrulamaları, gerçekten ayrı uygun profilli son sürüm incelemesi ve güncel CI yapılır; mevcut yetkiyle merge edilir, aynı main ağacı/tam main CI doğrulanır ve koordineli G28 kartı/kaynak pini bağlanır. Tasarım G28 kabulünü başarılı saymaz, atanmamış incelemeciyi bağımsız ilan etmez veya çalışma yetkisi vermez. Withdrawal, proje gizliliği, iş kabulü ve fiziksel resident model kapasitesi ayrı kabul kapılarında kalır.
