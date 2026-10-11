# Eylema — Geliştirme ve yerel işletim takımı protokolleri — v0.1

Dil: Türkçe. Kaynak sürüm: 0.1.0. İngilizce kaynak belge: [AI team protocols](ai-team-protocols-v0.1.en.md). Projenin API kimlikleri, alanları ve hata kodları İngilizce kalır.

Tarih: 11 Ekim 2026. Durum: uygulanacak protokol tasarımı; çalışan orkestratör, model seçimi, otonom PR üretimi veya zamanlanmış izleme kurulmuş değildir. Depo katkı şablonları ve CI kontrolleri [CONTRIBUTING.tr.md](CONTRIBUTING.tr.md) içinde ayrıca tanımlanır. Şema sürümü: 0.1.0. Örnekler gerçek görev, model veya ölçüm değildir.

## İki takım, ortak kanıt düzeni

**Geliştirme takımı:** Claude, ChatGPT/OpenAI, MiniMax gibi büyük model sağlayıcılarının görevlerine uygun modelleri; ihtiyaç çözümleme, mimari, kod, test, inceleme ve iyileştirme adayları üretir. Roller sağlayıcı adlarına sabitlenmez. Sağlayıcının kullanılabilir modeli, araç desteği ve maliyeti yapılandırmada doğrulanır.

**İşletim takımı:** Uygulamanın cihazında veya tanımlanmış yerel node’da çalışan uzman modeller ve Rust becerileri; kabul edilmiş model/araç/yetki paketi içinde günlük işleri yürütür. İşletim görevi kendiliğinden büyük bulut modele taşınmaz. Geliştirme için dış model kullanma izni, işletim verilerinin dışarı gönderilmesine genel izin vermez.

İki takım için de başarı, aynı sürüme bağlı çalıştırılmış kanıtla gösterilir. Bir modelin açıklaması test sonucu değildir. Protokolleri uygulayan Rust denetleyici durum geçişlerini ve araç yetkilerini kontrol eder; modeller bu denetleyiciye yapılandırılmış iş önerileri verir.

## A — İhtiyaçtan GitHub pull request’e geliştirme protokolü

### Roller

| Rol | Sorumluluk |
|---|---|
| Katkı sahibi | İhtiyacı, örnek davranışı ve kabul koşulunu tanımlar |
| İhtiyaç analisti / mimar | Gereksinimi yapılandırır, etkiyi ve iş planını çıkarır |
| Uygulayıcı | Kendi dalında kod, test ve gerekli belge değişikliklerini yapar |
| Doğrulayıcı | Araçları çalıştırır; sonuç ve ortam kanıtını toplar |
| Ayrı inceleyen | İhtiyaç, fark ve kanıtları yeni bir inceleme bağlamında değerlendirir |
| Rust koordinatör | Yetki, bütçe, durum, yeniden deneme, kanıt ve GitHub işlemlerini yönetir |
| Bakımcı | Ürün/mimari kararları ve repo politikasına göre birleştirme kabulünü yönetir |

Her işte bütün sağlayıcıların çağrılması gerekmez. Uygulayıcının kendi beyanı bağımsız inceleme sayılmaz. Farklı model aileleri kullanmak inceleme çeşitliliği sağlayabilir; doğruluk veya istatistiksel bağımsızlık garantisi vermez. Test beklentileri sırf mevcut kod geçsin diye gevşetilmez.

### Giriş, yetki ve bütçe

İhtiyaç web formundan, CLI’den veya GitHub issue’dan alınır. `NeedSpec` şu bilgileri taşır: protokol sürümü, görev kimliği/revizyonu, kaynak depo, amaç, örnek kabul senaryoları, kapsam, kısıtlar, hedefler ve yetki referansı.

Yetki, denetleyicinin doğruladığı kullanıcı kimliği ve depo politikasıyla belirlenir. Issue gövdesi, kod yorumu veya şemadaki `authorization_ref` metni tek başına yetki oluşturmaz. Yetkili katkıcının kabul edilmiş görevi, tanımlı sınırlar içinde geliştirme ve PR işlemlerine izin verir; rutin her adım için yeniden onay istenmez. Ürün niyetindeki gerçek eksik bilgi katkı sahibine sorulur.

Her çalışma model çağrısı, süre, maliyet/kullanım ve düzeltme döngüsü bütçesine sahip olur. GitHub olayının tekrar teslim edilmesi aynı işi ve ikinci bir PR’ı yeniden üretmemelidir. Depo + görev kimliği + revizyon, çalışmanın tekillik anahtarıdır.

### Belgelenmiş teslim nesneleri

| Nesne | Temel içerik |
|---|---|
| NeedSpec | İnsan ihtiyacı, kabul senaryoları, kapsam ve yetki referansı |
| WorkPlan | İhtiyaç revizyonu, başlangıç commit’i, iş düğümleri, roller, kontroller ve bütçe |
| ChangePackage | Değişiklik özeti, base/head commit, etkilenen düğümler ve kabul–test eşlemesi |
| Evidence | Kontrol, kod/model/veri sürümü, ortam, sonuç, komut/araç ve kayıt referansı |
| ReviewReport | İncelenen commit, bulgu kimlikleri, önem ve kapatılma kanıtı |
| PullRequestRecord | Depo, dal, görev ilişkisi, gerçek PR URL’si ve son doğrulama durumu |

Modelin iç düşünce zinciri bir teslim nesnesi değildir. Karar gerekçesi, yapılan araç çağrıları ve gözlenen sonuçlar yeterli ve incelenebilir düzeyde kaydedilir.

### Durum akışı

`received → spec_ready → planned → implementing → local_verification → independent_review → draft_pr → repository_checks → ready_for_maintainer`

Başarısızlık, ilgili aşamada gerekçeli düzeltme döngüsü başlatır. Eksik bilgi `needs_input`, bütçe sınırı `budget_exhausted`, iptal `cancelled` olarak görünür. Düzeltme sınırına ulaşılması başarı sayılmaz. İnceleme için taslak PR erken paylaşılabilir; eksik kontroller görünür kalır.

Her iş izole bir dal/çalışma alanında yürür. Paralel ajanlarda dosya sahipliği veya açık birleştirme noktası belirlenir. Planın kapsamı değişirse revizyon ve gerekçe kaydedilir. Depodaki aynı commit’e ait talimat ve kalite profili temel alınır.

### Kalite kapıları

1. **İhtiyaç uyumu:** Her kabul koşulu doğrulama yöntemiyle eşleşir; kapsam dışı değişiklikler ayrılır.
2. **Mimari ve bağımlılık:** Kendi Rust bileşenleri politikası ve modül sınırları korunur; doğrudan/geçişli üretim bağımlılıkları kontrol edilir.
3. **Rust doğruluğu:** İlgili hedeflerde biçim, derleme, statik kontroller ve davranış testleri tamamlanır.
4. **Gerçek davranış:** Değişen kullanıcı akışı, API, kayıt kurtarma veya eylemci davranışı uygun ortamda çalıştırılır.
5. **Performans/model kalitesi:** Etkilenen iş yükünde karşılaştırma yapılır. Değişiklik bunu gerektirmiyorsa gerekçeli `not_applicable` seçilir.
6. **İnceleme:** Engelleyici bulgular kapatılır; inceleme güncel değişiklik sürümüne bağlanır.
7. **PR bütünlüğü:** Depo CI sonucu ve kanıtlar son head commit’iyle eşleşir; gerekli birleştirme sonucu kontrolleri tamamlanır.

Kontrol sonuçları `passed`, `failed`, `skipped`, `not_run`, `not_applicable` olarak ayrılır. Zorunlu bir kontrolün `skipped` veya `not_run` olması hazır geçişini engeller. `not_applicable` modelin keyfî seçimiyle zorunluluğu kaldıramaz; sürümlü kalite profili karar verir.

Yeni commit, etkilenen inceleme ve kanıtları geçersiz kılar. Hedef dal değişmişse birleştirme davranışı da yeniden doğrulanır. Çapraz derleme, Linux’ta çalıştırma kanıtı; CPU fallback, GPU testi sayılmaz. Hızlı kontroller önce, pahalı tarayıcı/donanım testleri ilgili değişiklikler için sonra çalışır.

Kontrolleri veya yetki politikasını değiştiren PR, kendi yeni kurallarıyla kendine geçer not veremez. Denetleyici mevcut kabul edilmiş politika sürümünü kullanır; standart değişiklikleri ayrı değerlendirilir.

### GitHub teslimi

Önerilen entegrasyon bir GitHub App’tir; görev için izin verilen depoda dal ve PR işlemleri yapar. GitHub Apps otomasyon ve depo entegrasyonları için tasarlanmıştır. [GitHub Apps](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/about-creating-github-apps)

PR açıklaması problemi, yeni davranışı, kabul koşullarını, çalıştırılan kontrolleri, ortam sınırlarını ve varsa migration/geri dönüş yolunu içerir. Otomasyon, gerçek GitHub yanıtını almadan PR oluşturuldu saymaz; bilinmeyen sonuçta görev anahtarıyla mevcut PR aranır. Tekrarlanan çalışma aynı PR’ı günceller.

CI’nin PR açıldıktan sonra çalışacağı hesaba katılır: yerel kontrollerin ardından taslak PR açılır; depo kontrolleri ve inceleme tamamlanınca hazır işaretlenir. PR açılması birleştirme veya dağıtım yapıldığı anlamına gelmez. Gerekli kontroller ve incelemeler GitHub kurallarıyla uygulanabilir. [GitHub rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets)

Ajanın/test edilen kodun çalıştığı ortam ile GitHub yazma/model servis kimliklerini kullanan kontrol ortamı ayrılır. Public katkı kodu ayrıcalıklı workflow bağlamında çalıştırılmaz. Donanım testleri katkı sahibinin koduna bakımcının kişisel makine erişimini vermeden yürütülmelidir. [GitHub Actions güvenli kullanım](https://docs.github.com/en/actions/reference/security/secure-use)

## B — Donanıma göre yerel model takımı protokolü

### Sözleşmeler

| Nesne | İçerik |
|---|---|
| HardwareProfile | CPU/OS/browser, backend, desteklenen özellikler, gözlenen kapasite, profil sürümü |
| ResourceSnapshot | Mevcut yük, ayrılabilir kaynak tahmini, kuyruk, güç durumu, ölçüm zamanı |
| WorkloadProfile | Görev, dil/veri türü, kalite eşiği, context/çıktı sınırı, gecikme ve kaynak bütçesi |
| ModelCatalogEntry | Kaynak revision, ağırlık özeti, tokenizer, mimari, nicemleme ve motor uyumu |
| EvaluationReport | Model ve takımın gerçek görev/donanım sonuçları; ortam ve sürüm bağları |
| ModelSetRelease | Birlikte kabul edilmiş modeller, roller, profiller, seçenekler ve kanıtlar |
| AssignmentPlan | O cihazdaki rol–model–yürütücü eşleşmesi, yükleme planı, limitler ve gerekçe |

Takım; dil, algı, karar/politika, değer ve dünya modelleri ile deterministik Rust becerilerini kapsar. `behavior_contract_ref` ve içerik özeti; gözlem, durum, eylem, zaman, birimler, model bağlantıları ve reset/taşıma kurallarını sabitler. Dünya modeli ile politika birlikte değerlendirilir; dosyaların ayrı ayrı yüklenmesi ortak davranış uyumunu kanıtlamaz. Gözlenmiş durum, kestirim ve hayal edilen gelecek ayrı tutulur.

Ortak API’de HITL belirlenen eylemden önce insan kararını, HOTL izleme ve müdahaleyi, HIC hedef/bütçe/yetki yönetimini tanımlar. Rol adları kendi başına izin vermez. Gerekli onay tam plan/girdi, durum/politika sürümleri, kaynak kapsamı ve süreye bağlanır. Durdurma isteğinin alınması ile gerçekten durma ayrı sonuçtur; iptal geri alma değildir. Ayrıntılar [API referansındadır](api/tr/reference-v0.1.md).

Kalıcı donanım yetenekleri ile anlık kullanılabilir kaynaklar ayrı kaydedilir. Browser’ın göstermediği boş VRAM kesin bilgi gibi yazılmaz; bilinmeyen değer `unknown` kalır. Sürücü/backend/işletim sistemi değişikliği önceki uyumluluk kanıtının yeniden değerlendirilmesini gerektirebilir.

Model dosyasını okuyabilmek, modeli çalıştırabilmek değildir. Kendi Rust motorumuz mimariyi, gerekli işlemleri, veri tiplerini ve nicemleme biçimini gerçekten desteklemelidir. Safetensors gibi tensor dosya biçimleri bu yürütme motorunun yerine geçmez. [Safetensors tanımı](https://huggingface.co/docs/safetensors/index)

### Seçim sırası

1. Görev ve yerellik koşulu belirlenir; bazı görevlerin yalnız Rust becerileriyle çözülebileceği değerlendirilir.
2. Motor/operatör/format/donanım uyumu olmayan adaylar elenir.
3. Ayrılmış görev setinde asgari kalite ve yanlış eylem sınırını karşılamayan adaylar elenir.
4. Bellek, context, eşzamanlılık ve süre bütçesine sığan adaylar ölçülür.
5. Aday takım, ortak GPU/bellek ve UI yükü altında birlikte sınanır.
6. Profilin öncelik sırasına göre uygun takım seçilir: örneğin gecikme, enerji, bellek. Ölçülemeyen enerjiye tahminî sıfır değer verilmez.
7. Sonuç sürümlü takım paketine ve cihaz atama planına bağlanır.

En büyük veya en yeni model varsayılan kazanan değildir. “En verimli”, tanımlı aday kümesi, iş yükü ve donanım koşullarındaki ölçüm sonucudur; evrensel optimum iddiası değildir.

Takımda rol bulunması her rol için ayrı model gerektiği anlamına gelmez. Aynı model birden fazla rolü üstlenebilir; bütün modeller aynı anda bellekte tutulmayabilir. Hafif cihazda sıra ile yükleme; güçlü cihazda birlikte çalışma ancak ölçüm ve bütçe uygunsa seçilir.

### Ölçüm ve karşılaştırma

Dosya indirme, model yükleme, ilk sonuç, sıcak çalışma ve görev tamamlanma ayrı ölçülür. LLM görevlerinde giriş işleme/çıktı üretimi ve kısa/uzun context; diğer modellerde kendi görev ölçüleri kullanılır. Her role token/s ölçüsü uygulanmaz.

Bellek hesabı ağırlıklar, KV/durum, ara tensorler, çalışma alanı, eşzamanlı görevler ve arayüz/3D kaynaklarını kapsar. Düşük bitli nicemleme aynı cihazda otomatik daha hızlı sayılmaz. MLPerf Client gibi çalışmalar da kişisel bilgisayarlarda görev temelli ve uçtan uca AI ölçümü için referans sunar; kendi motorumuzun kabulü kendi gerçek görevleriyle yapılır. [MLPerf Client](https://mlcommons.org/benchmarks/client/)

Bir modelin tek başına çalışması, takımın birlikte çalışmasına kanıt değildir. Aday ve mevcut takım aynı görev seti ve karşılaştırılabilir ortamda sınanır. İşletim başarısı, hata, p95 süre, tepe bellek, yükleme sıklığı ve mümkünse enerji birlikte raporlanır.

### Sürüm ve atama

Dört sürüm ayrı izlenir: geliştirme protokolü, işletim protokolü, değişmez `ModelSetRelease` ve cihazın `AssignmentPlan` revizyonu. Kod sürümü, model dosyaları ve değerlendirme veri sürümü de kanıta bağlanır.

Sürüm paketinin içeriği değişmez; `candidate`, `approved`, `active` gibi yaşam döngüsü durumları ayrı kayıtlarla güncellenir. Şemadaki `status`, bu durumun görünümüdür; aynı sürüm kimliği altında model veya davranış sözleşmesi değiştirmeye izin vermez.

Rust yöneticisi onaylı takım içinden mevcut yük ve göreve uygun atama yapabilir. Bekleme, onaylı daha hafif modele geçiş veya açık başarısızlık davranışı önceden tanımlanır. Yeni model, farklı nicemleme, yeni motor veya yeni araç yetkisi takım sürümünü değiştiren adaydır; anlık atama diye gizlenmez.

Atama geçişlerinde gereksiz model yükle/boşalt salınımını önleyen bekleme süresi ve eşik bulunur. Devam eden görev model sürümüne bağlı kalır veya açık iptal/yeniden deneme kuralını izler. Yeni ve eski takımı aynı anda belleğe almanın sığacağı varsayılmaz.

### Güncel tutma döngüsü

`discovered → compatible_candidate → evaluated → release_pr → approved → canary → active`

Uygun olmayan aday `unsupported` veya `rejected`; etkin sürümden dönüş `rolled_back` olarak izlenir. `latest` etiketi çalışan modeli belirlemez; tam revision ve içerik özeti kullanılır.

Yeni kaynak sürümü, motor/backend değişikliği, yeni donanım profili veya görev kalitesindeki sapma değerlendirme işi oluşturabilir. Periyodik keşif de yapılandırılabilir. Sıklık, indirme ve değerlendirme bütçeleri depo/cihaz politikasıyla belirlenir; her cihaz her modeli sürekli denemez. Bu belge kapsamında zamanlanmış bir takip kurulmamıştır.

Geliştirme takımı aday model takımını ve gerekirse Rust motor değişikliklerini A protokolü üzerinden PR’a dönüştürür. Yüksek kalite kapısı için hazır testler aşağı çekilmez. PR birleşmesi dağıtımın gerçekleştiği anlamına gelmez; dağıtım ayrı sürüm yetkisini ve cihaz uyumunu kontrol eder.

Canary aşamasında küçük bir kullanım kümesi gözlenir. Kabul eşiği bozulursa önceki uyumlu takım geri etkinleştirilir. Yeni modelin dış dünyada gerçekleştirdiği eylemler veya değiştirdiği veriler sırf ağırlık geri alındı diye geri alınmış sayılmaz; sonuç/durum uyumluluğu ayrıca yönetilir. Offline cihaz en son uyumlu kabul edilmiş takımla çalışabilir; katalog ve kanıt güncelliği görünür tutulur.

## İlk uygulanacak ortak senaryo

Bir katkıcı görev sınıflandırmasının bir cihaz sınıfında yavaş olduğunu bildirir. Geliştirme takımı kabul senaryosunu belirler; mevcut küçük modeli ve daha hafif adayını aynı görevlerde karşılaştırır. Gerekirse kendi Rust hesaplama yolunu iyileştirir. Kod/model/politika değişikliği, bağımsız inceleme ve ölçümlerle taslak PR’a dönüşür. Depo kontrollerinden geçen paket kabul edilir; cihazdaki yönetici onaylı takımı deneyip rol atamasını günceller. Gecikme iyileşirken kalite geriliyorsa paket kabul edilmez.

İlk uygulama kanıtı; gerçek bir ihtiyaçtan gerçek PR’a giden geliştirme akışı ve en az iki kaynak profilinde açıkça farklı, ölçümle gerekçelendirilmiş işletim atamasıdır. Henüz bu akışlar çalıştırılmamış, cihaz/model seçimi yapılmamıştır.

## Makinece okunabilir başlangıç dosyaları

Öğrenme verisi ve Unsloth eğitim akışı [eğitim protokolünde](ogrenme-verisi-ve-egitim-protokolu-v0.1.md); insan katılımı, gözetimi ve yönetimi [API referansında](api/tr/reference-v0.1.md) tanımlanır. Eğitim araçları ayrı bağımlılık envanterindedir. Model atama yetkisi, eğitim verisini paylaşma veya yeni ağırlığı etkinleştirme yetkisi oluşturmaz.

- `ai-team-protocol/request.schema.json`: geliştirme isteğinin yapısal sözleşmesi.
- `ai-team-protocol/request.example.json`: örnek bir istek; gerçek yetki veya görev değildir.
- `ai-team-protocol/model-set-release.schema.json`: aday/kabul edilmiş yerel model takımının yapısal sözleşmesi.

JSON Schema yalnız yapısal doğrulama sağlar. Kimlik/yetki, model uyumluluğu, içerik özeti doğruluğu, cross-reference bütünlüğü, kanıtın güvenilir üreticisi ve durum geçişleri Rust denetleyicinin anlamsal kontrolleri olacaktır. Şema doğrulaması, framework’ün veya AI takımının çalıştığını kanıtlamaz.
