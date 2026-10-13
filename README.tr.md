<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/eylema-logo/logo-dark.png">
    <img src="assets/eylema-logo/logo.png" alt="Eylema" width="520">
  </picture>
</p>

# Eylema

**Basit, Performanslı Eylemci Yapay Zeka Sürekli Geliştirme ve İşletme Web Platformu.**

Kod, kimlikler, dosya adları ve katkı şablonları İngilizcedir. Dokümantasyon [İngilizce](README.md) ve Türkçe sunulur; kaynak dil İngilizcedir. **Eylema** ürün adı **11 Ekim 2026** tarihinde kabul edildi; yukarıdaki tanım ürünün amacını anlatır.

Herkese açık çekirdek proje deposu [eylema-dev/eylema](https://github.com/eylema-dev/eylema), [eylema-dev organizasyonundadır](https://github.com/eylema-dev). Paket öneki ve native host CLI `eylema`; yerel servis başlatma, görev çalıştırma/inceleme, servis durdurma ve doctor komutları uygulanmıştır. Görev iptali bağımsız olarak host tarafından doğrulanmış insan oturumu gerektirir. Proje sahibi 11 Ekim 2026 tarihinde **`eylema.dev` alan adını GoDaddy üzerinden kaydettiğini** bildirdi. Kendi kaynak kodumuz ve belgelerimiz [Apache-2.0](LICENSE) lisanslıdır. Ayrı belirtilen üçüncü taraf içerikleri, model ağırlıkları ve veri kümeleri kendi koşullarını korur.

`eylema-dev` organizasyonu ve bu public depo 11 Ekim 2026 tarihinde oluşturuldu. [İsim ve uygunluk kaydı](https://github.com/eylema-dev/devguide/blob/main/docs/tr/name-availability.md), organizasyon oluşturulmadan önceki kontrolleri ve sonraki alan adı güncellemesini korur. Ana alan adı `eylema.dev`; alan adı kaydı, platformun yayımlandığı veya kabul edildiği anlamına gelmez.

Marka dosyaları: [SVG / PNG logolar ve kullanım kılavuzu](assets/eylema-logo/README.tr.md), [önizleme](assets/eylema-logo/brand-preview.png), [logo paketini indir](assets/eylema-logo.zip).

**Durum: birinci taraf Rust görev çekirdeği, kalıcı kurtarma, kapsamlı native loopback HTTP servisi, native görev CLI’ı ve yerel tarayıcı görev konsolu.** [G16 tarayıcı konsolu notları](docs/tr/local-browser.md) aynı kaynaklı arayüzü, yenileme sonrası kurtarmayı ve doğru durdurma yetkisini açıklar. [G14 yerel HTTP kanıtı](docs/tr/local-http.md), gerçek soket/dosya davranışını, oturum/Host/Origin/CSRF kontrollerini ve sınırlı girdi/süre reddini kapsar. [G13 kurtarma kanıtı](docs/tr/runtime-recovery.md), korunan kimlik/sonuç/idempotency ile gerçek alt süreç kesintilerini, doğru belirsizlik sonucunu ve yeni kaydı kapsar. [G12 günlük kanıtı](docs/tr/local-journal.md), kanonik depolama, açık hatalar ve gerçek macOS senkronizasyon/kapat/yeniden aç akışını kapsar; anlamsal runtime yeniden başlatma kurtarması G13 ile kapsanır. [G11 denetim kanıtı](docs/tr/human-oversight.md), native davranışı ve açık simüle edilmiş insan host sınırını kaydeder. [G10 yürütücü kanıtı](docs/tr/deterministic-runtime.md) sınırlı native kabul/adım/olay ve gerçek sahipli durumu; [G09 codec/platform kanıtı](docs/tr/codec-platform.md) gerçek native/Node içinde Wasm karşılaştırmasını kapsar. [G07 workspace kanıtı](docs/tr/rust-workspace.md) gerçek native derleme/süreç testlerini ve seçilen taşınabilir kütüphanenin Wasm derlemesini kapsar. [G15 yerel CLI kanıtı](docs/tr/local-cli.md), alt süreç CLI/servis kimliği, hata, iptal yetkisi, kapanış ve günlük yeniden açma davranışını kapsar. Gerçek kişi doğrulaması/tarayıcı denetimi, model yürütümü, otonom geliştirme ve performans kabulü sonraki Goal’lerde kalır.

G20 WebGPU sahnesi notları: [yerel WebGPU görünümü](docs/tr/local-webgpu.md) sınırlı sahneyi, kontrolleri ve açık desteklenmiyor/aygıt kaybı durumlarını anlatır. Tarayıcı/GPU kabulü depo kontrollerinden ayrı kaydedilir.

G21, bildirilen Metal, görünüm ve model iş sınıfları için sınırlı, eşgüdümlü ana makine kaynak kabulü ekler: [kaynak yöneticisi](docs/tr/resource-manager.md). Bayt sınırları çağıranın bildirdiği geçici rezervasyonları kapsar; fiziksel GPU belleğini, model ağırlıklarını veya gerçek tarayıcı/model yürütümünü ölçmez.

G22, sınırlı [birleşik GPU ölçüm ana makineleri](docs/tr/combined-gpu-measurement.md) ekler; tekrarlanan gerçek donanım kabulü ve ham dağılımlar eylema-evals deposunda izlenir.

G39, [kontrollü yerel geliştirme çalışma alanı](docs/tr/development-workspace.md) ekler: kabul edilmiş tabana bağlı detached worktree, sınırlı tek süreçli macOS yalıtımı, kapsamlı artifactler ve kirli çağıran işinin korunması. Yerel kanıt sentetik host yetkisi kullanır; sağlayıcı/derleyici/GitHub yürütmesi ayrıdır.


## Katkıda bulunma

[Katkı kuralları](https://github.com/eylema-dev/devguide/blob/main/CONTRIBUTING.tr.md) ve ortak [AI eylemci sözleşmesi](https://github.com/eylema-dev/devguide/blob/main/AGENTS.md) ile başlayın. [İlk sürüm yol haritasını](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/roadmap.md), [kalite kontrollerini](https://github.com/eylema-dev/devguide/blob/main/docs/tr/quality-gates.md) ve [CI araç envanterini](https://github.com/eylema-dev/devguide/blob/main/docs/tr/tooling.md) izleyin. Issue/PR şablonları kapsam, kabul koşulları ve kanıt toplar. Bakımcı incelemesi CI’dan ayrıdır; bu dosyalar zorunlu kontrol dal kurallarını etkinleştirmez.

G46 taşınabilir [döngü, rol ve model bağlama sözleşmeleri](docs/tr/platform-contracts.md) ekler: değişmez tanımlı profiller ve sınırlı döngü tanımları. Native constructor kontrolleri model, eğitim ve harness iş kabulünden ayrıdır.

## Ürün yönü

İnsan hayal gücü ve uzman değerlendirmesinin büyük dil modellerinin desteğiyle sınırlı agentic sistemler kurduğu bir Rust harness platformu geliştiriyoruz. Kullanıcı değiştirilebilir yerel açık ağırlık model rollerini, RAG ve yeniden kullanılabilir skill'leri birleştirir; isteğe bağlı olarak harici büyük modeli açıkça bağlar. Seçimi performans, gizlilik ve ölçülmüş sonuçlar belirler. AI takımları platformu açık, incelenebilir sözleşmelerle geliştirir ve işletir. Uygulamalar durum, amaç, eylem, yetki, bütçe ve kabul koşullarını tanımlar; insan katılımı, gözetimi ve komutası sistemin pratik yetenekleri olarak korunur.

- Web, veri, eylemci, hesaplama ve araçlar için kendi Rust üretim bileşenlerimiz; seçilebilir 3B, yerel dil, karar ve dünya modeli yetenekleri.
- İlk deneysel sürümde tek M4 Pro/24 GB cihazda tarayıcı Rust/Wasm/WebGPU ve native macOS CPU/Metal; Linux ve ikinci fiziksel cihaz kabulü sonraki kapsamdır.
- İhtiyaçtan PR’a standart protokol kullanan büyük model geliştirme ekipleri.
- Donanıma göre seçilen, sürümlenen ve ortak uygulama yükünde değerlendirilen yerel model takımları.
- İzinli deneyim, insan düzeltmesi ve gerçek sonuçlardan kürasyonla oluşturulan eğitim verileri.
- Modele özel native Rust gradyan, LoRA/QLoRA ve kontrollü iyileştirme/terfi planlanır. Unsloth benzeri araçlar açıkça incelenmiş geliştirme/referans veya reçete yardımcılarıdır; özgün harici referans koşusu ve kendi runtime yeniden kabul kapıları ayrı kalır.
- Kod, veri, değerlendirme, eğitim tarifi, model ve donanım profili için açık katkı sözleşmeleri.

## “Dünyanın en iyisi” hedefinin kanıtı

Amaç AI ile web geliştirme ve işletmede öncü platformlardan biri olmaktır. Bu hedef mevcut bir sıralama veya her koşulda üstünlük sözü değildir. Kamuya açık karşılaştırmalar donanımı, iş yükünü, kalite eşiğini, sürümleri, yapılandırmayı, sınırları ve yeniden üretilebilir sonuçları paylaşmalıdır.

| Boyut | Aranacak kanıt |
|---|---|
| AI ile geliştirme | Ayrılmış değişiklik görevlerinde doğru tamamlama; düzeltme sayısı, süre, model maliyeti ve insan müdahalesi |
| Uygulama güvenilirliği | Uçtan uca davranış, kayıt/kurtarma, yetki, yükseltme ve açık hata sonuçları |
| Birlikte çalışma performansı | Sahne kare süresi p95/p99, eylem/görev gecikmesi, kalite, tepe bellek, başlangıç ve ölçülebilen enerji |
| Karar ve dünya modelleri | Kapalı döngü görev başarısı, kısıt ihlali, çok adımlı tahmin hatası ve deterministik başlangıca karşı sonuç |
| Sürekli öğrenme | Bağımsız görevlerde karşılaştırılabilir kaynak bütçesiyle iyileşme; gerilemeler ve başarısız adaylar dâhil |
| İnsan denetimi | Etkili onay, izleme, müdahale ve yetki geri alma; ölçülen müdahale sınırları ve anlaşılır kanıt |
| Basitlik ve genişletilebilirlik | İlk çalışan uygulama, kavram/API kapsamı, bağımsız katkı başarısı, hedefler ve bağımlılık envanteri |

Hız veya maliyet iyileştirilmeden önce asgari kalite karşılanır. Kernel yanında tamamlanmış uygulama sonuçları karşılaştırılır; soğuk ve sıcak çalışma ayrı raporlanır. Tek bir yapay puan ödünleşimleri gizlememelidir. Kıyas araçları ve referans uygulamalar üretim bağımlılığı sınırının dışındadır. Sayısal hedefler ilk iş yükü ve donanım seçimiyle belirlenir.

## Güncel belgeler

| Belge | İngilizce kaynak | Türkçe karşılık |
|---|---|---|
| Proje yapısı 0.5.0 | [First application and component proposal](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/project-structure.md) | [İlk uygulama ve bileşen önerisi](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/project-structure.md) |
| Mimari 0.6.0 | [Architecture](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/architecture.md) | [Mimari](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/architecture.md) |
| API ve denetim 0.3.0 | [API reference](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/api-reference.md) | [API referansı](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/api-reference.md) |
| Geliştirme ve işletim 0.2.0 | [Team protocols](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/development-and-operations.md) | [Takım protokolleri](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/development-and-operations.md) |
| Öğrenme ve eğitim 0.2.0 | [Learning protocol](https://github.com/eylema-dev/eylema-docs-en/blob/main/docs/learning-and-training.md) | [Öğrenme protokolü](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/learning-and-training.md) |

Makinece okunabilir taslaklar: [geliştirme isteği şeması](ai-team-protocol/request.schema.json), [örnek istek](ai-team-protocol/request.example.json), [macOS model-set 0.2.0 şeması](ai-team-protocol/v0.2/model-set-release.schema.json), [sentetik aday](ai-team-protocol/v0.2/model-set-release.example.json), [korunmuş tarihsel 0.1.0 şeması](ai-team-protocol/model-set-release.schema.json). Şemaya uygunluk, yetki veya başarılı çalışma kanıtı değildir.

İşlem kimlikleri, yollar, alanlar, enum değerleri ve hata kodları iki dilde aynı İngilizce tanımları kullanır. Güncel belgeler `eylema-docs-en` ve `eylema-docs-tr` depolarının `docs/` dizinlerinde eşleşen İngilizce dosya adları taşır; çeviriler kaynak sürümünü izler. Önceki Türkçe mimari sürümleri ve ilk API özeti tarihsel kayıt olarak `eylema-docs-tr/archive/` altında korunur. Düzen ve taşıma notları [proje yapısı belgesindedir](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/project-structure.md).

## İlk kanıt

Seçilen ilk çalışan uygulama bir görev konsoludur: görev oluşturma, deterministik adımları çalıştırma, ilerlemeyi inceleme, kalan işi durdurma, durumu kaydetme ve hizmet yeniden başladıktan sonra tekrar açma. Önce Rust sözleşmeleri, yürütücü ve günlük kurulur, ardından asgari web arayüzü gösterilir. [Kabul senaryoları ve önerilen bileşenler](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/project-structure.md), gösterimin neyi kanıtlaması gerektiğini tanımlar.

Sonraki 3B ortam, öğrenilmiş politika/dinamik modeli, yerel dil modeli, izinli eğitim döngüsü ve geliştirme takımının PR akışı ayrı yol haritası aşamalarıdır. İlk tarayıcı gösterimi model erişimi veya GPU gerektirmez. G15 CLI ve G16 tarayıcı konsolu artık yerel görev hizmetini çalıştırıp inceler; insan olarak doğrulanmamış CLI oturumundaki durdurma isteği doğru biçimde `FORBIDDEN` döndürür.

## Organizasyon depoları

[İngilizce ürün belgeleri](https://github.com/eylema-dev/eylema-docs-en) · [Türkçe ürün belgeleri](https://github.com/eylema-dev/eylema-docs-tr) · [Geliştirici rehberi](https://github.com/eylema-dev/devguide) · [Depo haritası](https://github.com/eylema-dev/devguide/blob/main/docs/tr/repository-map.md)

`eylema`, mevcut Rust görev çekirdeği, servis/CLI/tarayıcı bileşenleri, platform sözleşmeleri, makine şemaları ve marka varlıklarını sahiplenir. Ürün metinleri dil depolarında, ortak katkı politikası devguide'dadır. Örnek/değerlendirme depoları kabul edilmiş consumer ve sınırlı ölçüm kanıtı içerir; model/eğitim depoları kendi yürütüm kapılarını bekler. Web sitesi [eylemadotdev](https://github.com/eylema-dev/eylemadotdev) içinde ayrı yönetilir. Depo bölünmesi tek başına yürütüm kanıtı değildir; bileşen notları gerçek kabul kapsamını bağlar.

[Taşıma kaydı](https://github.com/eylema-dev/devguide/blob/main/docs/tr/repository-migration.md) kaynakları, hedefleri, ilişkili PR'ları, entegrasyon kontrollerini ve geri dönüşü açıklar. Yeni belge bağlantıları eşgüdümlü birleştirme sonrası yerleşimi hedefler; öncesinde PR'lar ve sabitlenmiş depo kümesi inceleme kopyalarını sağlar.

Bakımcı 12 Ekim 2026 tarihinde [ilk ürün kararları ve test envanterini](https://github.com/eylema-dev/eylema-docs-tr/blob/main/docs/project-structure.md) kabul etti: ilk kullanıcı yerel, tek kullanıcılı eylemci uygulaması geliştiren yazılımcıdır. Kullanılabilir görev konsolu önizlemesi ve erken ihtiyaçtan PR’a kanıt, isteğe bağlı model/eğitimden önce gelir. MacBook Pro M4 Pro/24 GB birincil kalır; Mac mini M4/24 GB ve Linux RTX 4070 Ti Super/16 GB VRAM + 32 GB RAM kendi yürütüm kanıtını bekleyen ek test cihazlarıdır. Mevcut Rust tipleri, EYL/EYLJ biçimleri ve sınırlı platform bağları uygulama temeli olarak korunur.

- [G38 geliştirme koordinatörü](docs/tr/development-coordinator.md): Projeye ait Rust ihtiyaçtan incelemeye durum akışı, host tarafından çözümlenen yetki, sonlu bütçe/iptal ve eski kanıt reddi; süreç/sağlayıcı/GitHub izolasyonu ve gerçek teslim G39–G43 adımlarıdır.
