# Beyan edilen kaynak tahmini

G80, mevcut [kaynak yöneticisinin](resource-manager.md) yanına kendi Rust kodumuzdan oluşan saf bir boyut hesaplama modülü ekler. [English](../en/resource-estimator.md). [Onaylanan tasarım](g80-resource-estimator-design.md) kapsamı, [teknik plan](https://github.com/eylema-dev/devguide/blob/main/docs/plans/g80-resource-estimator.md) uygulama ve kabul kapılarını belirler.

## Çağıranın sözleşmesi

`eylema_runtime::resource_estimate::component_interval` tek beyanı hesaplar; `estimate`, ödünç alınan `Snapshot` girdisini sabit kapasiteli `EstimateReport` içinde birleştirir. İkisi de heap/cihaz belleği ayırmaz, lease almaz, donanım sorgulamaz, kabul davranışını değiştirmez, ağırlık indirmez, model/eğitim çalıştırmaz, provider çağırmaz, iş yönlendirmez veya ekipman satın almaz. Mevcut görev/EYL/journal/host arayüzleri korunur. Native çalıştırma uygunluğu, ölçülmüş performans, kalibrasyon ve yatırım önerileri ayrı kapılardır; `planning_only` her zaman true olur.

Çağıran gereken bütün bileşenleri, katsayıları, yerleşimleri ve örtüşmeleri sağlar. Bu API atlanan gereksinimleri keşfedemez, yayıncı/donanım/kaynak kimliğini doğrulayamaz. Tam beyan, tam fiziksel ölçüm değildir. Eksik katsayı için `Claim::Unknown`, gerçekten uygulanmayan bileşen için açıkça `Formula::NotApplicable` kullanılır. Zorunlu yerleşik ağırlıklar uygulanmaz sayılamaz. Sıfır metaveri veya rezerv açık ve bilinen değer olabilir; eksik bilgi sıfıra dönüşemez.

Her bilinen formül/aralık `Evidence` taşır: köken (`MeasuredArtifact`, `PublisherDeclaration`, `SuppliedAssumption`), sınırlı kaynak referansı, pozitif revizyon ve uçları dahil geçerlilik aralığı. Karma kökenler ayrı bileşen veya iç metaveri beyanı gerektirir. Rapor, bileşen kanıtını ve özgün formül/iç köken bilgileriyle değiştirilemez ödünç snapshot'ı korur. İstenen context ve batch boyutlandırma ayarlarıdır; ölçülen kullanımı anlatmaz. Etiket, profil adı, parmak izi ve tahmin hak, yetki veya güven sağlamaz.

`InstanceKey`, profil, revizyon, artifact özeti, backend, sahip kapsamı ve fiziksel havuzu içerir. Her yerleşik örnek tam `expected_key` değerine; snapshot kimliği/revizyonu da beklenen değerlere bağlanır. Farklı roller yalnız bu tam örneği paylaştıklarında tek yerleşik ID kullanır. Aynı örneğin tekrar tanımı belirsiz tekilleştirmeyi reddeder; farklı sahip/artifact/revizyon/backend/profil/havuz ayrı sayılır. Her çağrının durum/workspace belleği ayrıdır. Eksik artifact kimliği, eski kanıt veya istenen kimlikle uyuşmazlık kesin sığma sonucunu engeller.

## Açık formüller

Birimler tipli bit/bayt/adet/saniyedir. Tüm hesaplar denetimli `u64` ara değerleriyle yürür; kayan nokta, sarma veya saturasyon kullanılmaz. Daha geniş aritmetik son sonucu temsil edebilse bile ara taşma hatadır. Bayt paketleme ve rasyonel ölçekleme, alt sınır dahil yukarı yuvarlanır. Aralıklar muhafazakâr beyan sınırlarıdır; istatistiksel güven aralığı değildir.

| Formül | Kural ve gereken beyanlar |
| --- | --- |
| `Packed`, `Weights` | Sürekli bit paketleme: ceil(eleman × bit / 8). Ağırlıklar açık metaveri sınırını ekler ve toplam yerleşik parametreyi kullanır; aktif MoE parametreleri toplamın yerine geçmez. |
| `Quantized` | Paketlenmiş parametre baytı + ceil(parametre / group_size) × scale_bytes + metaveri sınırları. Son kısmi grup sayılır; satır hizalama, ek codebook/zero point ve kaynak/hedef/staging açık boyut ister. Desteklenmeyen NF4/özel düzen bit sayısından çıkarılmaz, bilinmeyen kalır. |
| `Kv`, `Recurrent` | İstenen tam context × batch × katman × KV başlığı × başlık genişliği × 2; ardından cache bitleri paketlenir. Recurrent kendi state-elemanı/batch/precision ve açık rasyonel katsayısını kullanır; dense-cache'e düşmez. |
| `RagVectors` | Adet × boyut × precision, ardından paketleme; index, tutulan kaynak, çalışma tamponu ve kalıcı depolama ayrıdır. |
| `Lora` | Denetlenmiş hedefler için rank × (input + output) toplamı; ardından adaptör bit paketlemesi ve açık copies oranı. Rank iki boyuta da sığmalıdır. Gradient/optimizer/base/activation/checkpoint/temp ayrıdır; evrensel katsayı yoktur. |
| `Bytes`, `Scaled` | Çağıranın bayt aralığı, isteğe göre açık pozitif rasyonel katsayıyla çarpılır ve uçlar yukarı yuvarlanır. Modality/workspace/index durumu denetlenmiş veya varsayılan açık aralık ister. |

`Quantized`, beyan edilen paketleme formülüdür; ağırlık biçimini tanıma veya destekleme değildir. QLoRA boyutlandırması, beyan edilmiş quantized tabanı ayrı adaptör/gradient/optimizer/activation/checkpoint/temp gereksinimleriyle birleştirir. Dönüşüm giriş/çıkış/staging bileşenleri ömür ve fiziksel havuzlarını belirtir; aynı yerleşik ayırmayı ayrıca başka auxiliary adıyla eklemeyin. Hesaplayıcı ilgisiz iki bileşen ID'sinin aynı ayırma olduğunu çıkaramaz. Görüntü/video/STT/TTS tamponları açık yardımcı bileşenlerdir; yeni mimariler açık recurrent/beyan katsayısı veya `UnsupportedArchitecture` kullanır. Seçilmiş Qwen profilleri dahil kernel, eğitim uygulaması veya model desteği ima edilmez.

## İşbirliği ve fiziksel bellek

En fazla16 yerleşik örnek,32 çağrı,32 örtüşme grubu ve64 yardımcı bileşen kullanılır; uygulama ayrıca8 havuz,grup başına32 üye ve32 LoRA hedefiyle sınırlıdır. Çıktı en fazla144 bileşen tutar. Girdi ID'leri bütün girişler arasında; grup ve havuz ID'leri kendi ad alanlarında benzersizdir. Kimlik/kaynak referansları boş olmayan, en fazla128bayt ASCII ve harf/rakam ile `._:/@-` karakterlerinden oluşur. Açıkça bilinmeyen eksik kimlik boş kaynak referansı taşıyabilir. Serbest metin/erişim bilgisi gerekmez.

Her havuz yerleşik ağırlık ve kalıcı yardımcıları bir kez toplar; ardından açık gruplar arasındaki çağrı/geçici yardımcı toplamlarının en yüksek alt ve üst sınırlarını ekler. Bir üye alternatif gruplarda bulunabilir; aynı grupta tekrarlanamaz. Her çağrı/geçici yardımcı en az bir grupta bulunmalıdır; kalıcı yardımcı gruba üye olamaz. Eksik çizelge (`None`) tüm havuzları bilinmeyen yapar. Açık boş çizelge yalnız geçici iş yoksa geçerlidir. Eksik üyeli çizelge etkilenen tepe değerini bilinmeyen bırakır. Eşzamanlı durumlar paylaşılan ağırlıklardan ayrıdır.

Unified RAM/VRAM tek fiziksel havuzdur. Ayrık host RAM/cihaz VRAM ayrı fiziksel ID taşır; tekrarlanan fiziksel tanımlar alias ile çift sayımı reddeder. Depolama bellekten ayrıdır. Kapasite, brüt güncel kullanılabilir bayt ve OS/allocator rezervi ayrı beyanlardır: kullanılabilir bütçe = available − reserve. Kapasite tek başına güncel sığmayı göstermez; eski/eksik capacity/available/reserve bütçe üretmez. Kapasiteyi aşan available veya available'ı aşan reserve çelişkili beyandır.

| Sonuç | Anlam |
| --- | --- |
| `EstimatedFit` | Tam üst sınır her havuzdaki bilinen kullanılabilir bütçeye <= olur. |
| `ExceedsDeclaredBudget` | En az bir havuzda tam alt sınır bilinen kullanılabilir bütçeyi aşar; başka havuz açığı kapatamaz. |
| `Unknown` | Bütçeyi kesen aralık, zorunlu katsayı/çizelge/bütçe eksikliği, eski kanıt veya kimlik uyuşmazlığı. Bilinen bileşen alt toplamı görünür kalır; eksik toplam sıfır değil `None` olur. |

Genel kesin yetersizlik bilinmeyen havuzlardan önce gelir; yoksa herhangi bir bilinmeyen sığmayı engeller. `snapshot_status`, snapshot kimlik/çizelge belirsizliğini taşır. Havuz bütçesi nedenleri ödünç özgün havuz beyanlarından incelenebilir. Geçersiz birim/şekil/payda/sınır/tekrar/çözülemeyen referans/çelişki ve aritmetik taşma `EstimateError` döndürür; belirsizlik bozuk girdiden ayrıdır. Bu, kaynak doğrulaması veya kabul garantisi değil beyan tutarlılığıdır.

## Bağımsız sentetik örnek

Kullanılabilir bütçesi1000 olan tek unified havuz ve ayrı400 depolama bütçesi:

- Quantized taban12..14bayt:10parametre,4bit,group4,scale2bayt,metadata1..3bayt.
- Kalıcı adaptör40,gradient40,optimizer80,recurrent72,vector48,index16bayt.
- Örtüşen tek çağrı:activation100/workspace10; checkpoint20,modality8,RAG-working4bayt.
- Depolamada tutulan kaynak300bayt.

Sabit beklenen bellek450..452bayt, depolama300bayttır; iki tahmin de sığar. Eksik optimizer katsayısı belleği `None`/`Unknown` yaparken depolama ayrı sonucunu korur. Bu küçük sentetik değerler faydalı model çalışmasını veya deneysel kaynak boyutunu değil aritmetik/birleştirmeyi kanıtlar. Başka fixture tek1000bayt yerleşik örneği iki bağımsız çağrıya paylaştırır: ayrı gruplar1220bayt, ortak grup1330bayt tepe üretir. İki ayrı sahip örneğinde tepe2330bayt olur.

## Doğrulama sınırları

Native fixture'lar gerçek modülü bağımsız sabit beklentilerle çalıştırır: dense/MoE, paketleme/quantized son grup/rasyonel yuvarlama, KV/recurrent, RAG/adaptör/eğitim beyanları, eksik/eski/geçersiz/taşma, tam kimlik/paylaşım, örtüşme, unified/ayrık/depolama, bütçeyi kesme ve en yüksek/reddedilen sınırlar. Mevcut kabul testleri lease/iptal davranışını korur. Tam repository/native kapıları, tam commit ayrı incelemesi ve güncel CI/main kanıtı teslim PR'ı/güncel G80 kartında kaydedilir. Statik belge kontrolleri bu çalışma sonuçlarını tek başına kanıtlamaz.

G67'de daha önce gözlenen değişmemiş development-host worker başlangıç zaman aşımı tekrarlanabilirlik riski olarak korunur; G80 eşik/test/kaynak düzeltmesi veya muafiyet içermez. Fiziksel ayırma, gerçek model/ağırlık/tokenizer, GPU/model/eğitim performansı, tam iş yükü keşfi, Türkçe retrieval kalitesi, kalibrasyon ve yatırım sonuçları bu boyutlandırma modülüyle kabul edilmez.
