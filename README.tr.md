# Eylema — Tasarım Paketi

**Basit, Performanslı Eylemci Yapay Zeka Sürekli Geliştirme ve İşletme Web Platformu.**

Projenin kaynak dili [İngilizce](README.md), ikinci dili Türkçedir. **Eylema** ürün adı **11 Ekim 2026** tarihinde kabul edildi; yukarıdaki tanım ürünün amacını anlatır.

Herkese açık dokümantasyon deposu [eylema-dev/eylema](https://github.com/eylema-dev/eylema), [eylema-dev organizasyonundadır](https://github.com/eylema-dev). Önerilen CLI adı ve paket öneki `eylema`; henüz uygulanmış veya yayımlanmış değildir. Alan adı kaydı ve lisans seçimi henüz tamamlanmadı.

`eylema-dev` organizasyonu ve bu public depo 11 Ekim 2026 tarihinde oluşturuldu. [İsim ve uygunluk kaydı](name-availability.tr.md), organizasyon oluşturulmadan önceki kontrolleri korur. Önerilen ana alan adı `eylema.dev`; bu çalışmada alan adı satın alınmadı.

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
| Mimari 0.3.0 | [Architecture](architecture-v0.3.en.md) | [Mimari](framework-mimari-taslagi-v0.3.md) |
| API ve denetim 0.1.0 | [API reference](api/en/reference-v0.1.md) | [API referansı](api/tr/reference-v0.1.md) |
| Geliştirme ve işletim 0.1.0 | [Team protocols](ai-team-protocols-v0.1.en.md) | [Takım protokolleri](ai-takim-protokolleri-v0.1.md) |
| Öğrenme ve eğitim 0.1.0 | [Learning protocol](learning-data-training-protocol-v0.1.en.md) | [Öğrenme protokolü](ogrenme-verisi-ve-egitim-protokolu-v0.1.md) |

Makinece okunabilir taslaklar: [geliştirme isteği şeması](ai-team-protocol/request.schema.json), [örnek istek](ai-team-protocol/request.example.json), [model takımı sürüm şeması](ai-team-protocol/model-set-release.schema.json). Şemaya uygunluk, yetki veya başarılı çalışma kanıtı değildir.

İşlem kimlikleri, yollar, alanlar, enum değerleri ve hata kodları iki dilde aynı İngilizce tanımları kullanır. Çeviriler kaynak sürümünü izler. Eski v0.1/v0.2 mimari dosyaları ve ilk Türkçe API özeti tarihsel tasarım kayıtlarıdır; güncel belgeler bu dizinde belirtilmiştir.

## İlk kanıt

Kendi web arayüzümüz, Rust yürütmesi ve kalıcılığıyla küçük bir 3B görev ortamı geliştirilecek. Deterministik karar başlangıcının ardından küçük öğrenilmiş politika/dinamik modeli ve desteklenen bir yerel dil modeli ortak GPU yükünde değerlendirilecek. İzinli bir deneyim ve düzeltme kaydedilecek; sabitlenmiş tarifle eğitilen aday ancak iyileşme kanıtından sonra etkinleştirilecek. Geliştirme takımının gerçek, doğrulanmış bir PR üretmesi ve iki kaynak profilinde gerekçeli model ataması göstermesi gerekecek.

Her adım ayrı kabul aşamasıdır. İlk üründe bütün model aileleri, CAD çekirdeği veya her native GPU backend’i tamamlanmış olmak zorunda değildir.
