<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/eylema-logo/logo-dark.png">
    <img src="assets/eylema-logo/logo.png" alt="Eylema" width="520">
  </picture>
</p>

# Eylema

**Basit, Performanslı Eylemci Yapay Zeka Sürekli Geliştirme ve İşletme Web Platformu.**

Kod, kimlikler, dosya adları ve katkı şablonları İngilizcedir. Dokümantasyon [İngilizce](README.md) ve Türkçe sunulur; kaynak dil İngilizcedir. **Eylema** ürün adı **11 Ekim 2026** tarihinde kabul edildi; yukarıdaki tanım ürünün amacını anlatır.

Herkese açık dokümantasyon deposu [eylema-dev/eylema](https://github.com/eylema-dev/eylema), [eylema-dev organizasyonundadır](https://github.com/eylema-dev). Önerilen CLI adı ve paket öneki `eylema`; henüz uygulanmış veya yayımlanmış değildir. Proje sahibi 11 Ekim 2026 tarihinde **`eylema.dev` alan adını GoDaddy üzerinden kaydettiğini** bildirdi. Lisans seçimi henüz tamamlanmadı.

`eylema-dev` organizasyonu ve bu public depo 11 Ekim 2026 tarihinde oluşturuldu. [İsim ve uygunluk kaydı](docs/tr/name-availability.md), organizasyon oluşturulmadan önceki kontrolleri ve sonraki alan adı güncellemesini korur. Ana alan adı `eylema.dev`; alan adı kaydı, platformun yayımlandığı veya kabul edildiği anlamına gelmez.

Marka dosyaları: [SVG / PNG logolar ve kullanım kılavuzu](assets/eylema-logo/README.tr.md), [önizleme](assets/eylema-logo/brand-preview.png), [logo paketini indir](assets/eylema-logo.zip).

**Durum: mimari/API taslakları ve katkı araçları.** Depo CI kontrolleri belgeleri, şemaları ve PR yapısını doğrular; Rust runtime, model yürütmesi, otonom AI geliştirme takımı veya performans kıyası sağlamaz.

## Katkıda bulunma

[Katkı kuralları](CONTRIBUTING.tr.md) ve ortak [AI eylemci sözleşmesi](AGENTS.md) ile başlayın. [İlk sürüm yol haritasını](ROADMAP.tr.md), [kalite kontrollerini](docs/tr/quality-gates.md) ve [CI araç envanterini](docs/tr/tooling.md) izleyin. Issue/PR şablonları kapsam, kabul koşulları ve kanıt toplar. Bakımcı incelemesi CI’dan ayrıdır; bu dosyalar zorunlu kontrol dal kurallarını etkinleştirmez.

## Ürün yönü

AI ekiplerinin açık ve incelenebilir sözleşmelerle geliştirebildiği ve işletebildiği uçtan uca bir web platformu. Uygulamalar durumunu, hedeflerini, eylemlerini, yetkilerini, bütçelerini ve kabul koşullarını tanımlar. İnsan katılımı, gözetimi ve komutası aynı sistemin kullanılabilir yetenekleridir.

- Web, veri, eylemci, hesaplama ve araçlar için kendi Rust üretim bileşenlerimiz; seçilebilir 3B, yerel dil, karar ve dünya modeli yetenekleri.
- Ayrı doğrulanan yeteneklerle tarayıcı Rust/Wasm/WebGPU ve native Linux yürütmesi.
- İhtiyaçtan PR’a standart protokol kullanan büyük model geliştirme ekipleri.
- Donanıma göre seçilen, sürümlenen ve ortak uygulama yükünde değerlendirilen yerel model takımları.
- İzinli deneyim, insan düzeltmesi ve gerçek sonuçlardan kürasyonla oluşturulan eğitim verileri.
- Eğitim/paketleme hattında açık dış araç olarak Unsloth; model kabulünden önce kendi motorumuzda doğrulama.
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
| Proje yapısı 0.1.0 | [First application and component proposal](docs/en/project-structure.md) | [İlk uygulama ve bileşen önerisi](docs/tr/project-structure.md) |
| Mimari 0.3.1 | [Architecture](docs/en/architecture.md) | [Mimari](docs/tr/architecture.md) |
| API ve denetim 0.1.0 | [API reference](docs/en/api-reference.md) | [API referansı](docs/tr/api-reference.md) |
| Geliştirme ve işletim 0.1.0 | [Team protocols](docs/en/development-and-operations.md) | [Takım protokolleri](docs/tr/development-and-operations.md) |
| Öğrenme ve eğitim 0.1.0 | [Learning protocol](docs/en/learning-and-training.md) | [Öğrenme protokolü](docs/tr/learning-and-training.md) |

Makinece okunabilir taslaklar: [geliştirme isteği şeması](ai-team-protocol/request.schema.json), [örnek istek](ai-team-protocol/request.example.json), [model takımı sürüm şeması](ai-team-protocol/model-set-release.schema.json). Şemaya uygunluk, yetki veya başarılı çalışma kanıtı değildir.

İşlem kimlikleri, yollar, alanlar, enum değerleri ve hata kodları iki dilde aynı İngilizce tanımları kullanır. Güncel belgeler `docs/en/` ve `docs/tr/` altında eşleşen İngilizce dosya adları taşır; çeviriler kaynak sürümünü izler. Önceki mimari sürümleri ve ilk API özeti tarihsel kayıt olarak `docs/archive/` altında korunur. Düzen ve taşıma notları [proje yapısı belgesindedir](docs/tr/project-structure.md).

## İlk kanıt

Seçilen ilk çalışan uygulama bir görev konsoludur: görev oluşturma, deterministik adımları çalıştırma, ilerlemeyi inceleme, kalan işi durdurma, durumu kaydetme ve hizmet yeniden başladıktan sonra tekrar açma. Önce Rust sözleşmeleri, yürütücü ve günlük kurulur, ardından asgari web arayüzü gösterilir. [Kabul senaryoları ve önerilen bileşenler](docs/tr/project-structure.md), gösterimin neyi kanıtlaması gerektiğini tanımlar.

Sonraki 3B ortam, öğrenilmiş politika/dinamik modeli, yerel dil modeli, izinli eğitim döngüsü ve geliştirme takımının PR akışı ayrı yol haritası aşamalarıdır. İlk tarayıcı gösterimi model erişimi veya GPU gerektirmez. Bu dokümantasyon revizyonu çalışan uygulama sağlamaz.
