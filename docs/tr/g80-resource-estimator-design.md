# G80 kaynak tahmin katmanı — insan incelemesi için yazılı tasarım

Tarih:14Ekim2026. Durum:yazılı tasarım onaylandı; saf boyutlandırma yerelde uygulandı, son teslim kabulü bekliyor. [İngilizce kanonik kaynak](../en/g80-resource-estimator-design.md).

## Amaç ve mevcut temel

Planlı G80 kontrollü kaynak tahmin katmanını kendi Rust kodumuzla uygula. Eyleme kullanıcıları resident/aktif parametre, hassasiyet, context, batch, eşzamanlılık, RAG ve modele özel eğitim ihtiyaçlarıyla yerel uzman portföylerini karşılaştırabilsin. İzlenebilir bellek/depolama aralıkları ve bilinmeyenler üretilsin; gerçek performans kalibrasyonu ve yatırım önerileri G81/G82’de kalır. [Kabul edilmiş planlama yönü](https://github.com/eyleme-dev/devguide/blob/main/docs/tr/model-resource-planning.md) ve [G80 kartı](https://github.com/eyleme-dev/devguide/blob/main/records/platform-goal-plan.json) özgün A1/CP1/R1, bağımlılık ve bütçeleri korur.

Mevcut [kaynak yöneticisi](../../crates/eylema-runtime/src/resource.rs), çağıranın ilan ettiği geçici rezervasyonları, kuyruk/aktif sınırları ve işbirlikçi iptali izler. Kabul edilmiş model manifest’i boyut/extent doğrular ve desteklenmeyen yürütmeyi bildirir. İkisi de tam resident model veya eğitim ihtiyacını öngörmez. Tasarım bu temele ayrı, saf bir boyut hesaplama katmanı ekler. Mevcut kabul davranışı, EYL/task/journal sürümleri ve yetkilendirme korunur.

## Ele alınan yaklaşımlar

| Yaklaşım | Ödünleşim | Karar |
| --- | --- | --- |
| Kaynak yöneticisinin yanında kendi saf tahmin katmanımız | Türlenmiş deterministik hesaplama allocation/model/cihaz olmadan sınanır; çağıran sonucu açıkça kullanır | Önerilen |
| Tahmini doğrudan kaynak kabulüne eklemek | Daha az çağrı sınırı; belirsiz tahminleri mevcut lease/iptal davranışına bağlar | Kabul mevcut sınırlı yürütme sözleşmesi olarak korunur |
| Bütün hesapları model manifest’ine koymak | Artifact kimliğine yakın; portföy/RAG/eğitim/havuz mantığı manifest’in beyan sorumluluğunu aşar | Manifest kimlik/boyut sağlar; tahmin katmanı boyutlandırmayı üstlenir |

## Girdiler, kimlik ve sonlu sınırlar

Planlama snapshot’ı açık fiziksel bellek havuzları, model instance’ları, çağrı/eşzamanlılık grupları ve isteğe bağlı RAG/eğitim/depolama bileşenlerini içerir. Her değerin birimi ve kökeni vardır:ölçülmüş artifact, yayıncı beyanı, verilen varsayım veya bilinmeyen. İstenen ayar ölçülmüş kullanımdan ayrılır. Kaynak/model dizgesi, profil durumu veya tahmin çıktısı yürütme/veri erişimi yetkisi vermez.

Önerilen sınırlar:en fazla16resident model instance,32çağrı,32açık eşzamanlılık grubu ve64yardımcı bileşen. Kimlikler ve referans profilleri sınırlı, sürümlü ve tekil çözülür. Instance anahtarı profil/artifact parmak izi, sürüm, backend, sahip kapsamı ve fiziksel havuzu içerir; paylaşılan ağırlık yalnız aynı ilan edilen instance için bir kez sayılır. Ayrı context/activation durumları ayrıdır. Roller arasında ağırlık paylaşımı, cache veya projeler arası paylaşım varsaydırmaz.

Güncellik kontrolü tam istenen snapshot/beklenen profil sürümünü ve açık geçerlilik aralığını bağlar. Bilinmeyen, eksik, eski, çelişkili veya uyuşmayan kimlik kesin sığma sonucu üretemez. Bunlar beyan tutarlılığı kontrolleridir; yayıncı/donanım/insan iznini doğrulamaz.

## Kontrollü hesap ve belirsizlik

| Bileşen | Gerekli açık girdiler ve kural |
| --- | --- |
| Dense ve MoE ağırlık | Toplam resident parametre, denetlenmiş bit/packing ve metaveri aralığı; aktif MoE parametre hesap kapsamını anlatır, toplam resident ağırlığın yerine geçmez |
| Quantized saklama/dönüşüm | Açık grup/scale/metaveri ve kaynak/hedef/staging residency; desteklenmeyen layout veya eksik overhead ilgili bileşeni bilinmeyen bırakır |
| KV/recurrent durum | İstenen tam context/batch, layer, K/Vhead/headwidth ve cache hassasiyeti; recurrent durum kendi açık formül/katsayı profilini taşır |
| Workspace/modalite | Çağrı başına denetlenmiş veya verilmiş aralıklar; görüntü/video/STT/TTS tamponları açık yardımcı gereksinimdir, evrensel çarpan yoktur |
| RAG | Vektör sayısı/boyutu/hassasiyeti; index, tutulan kaynak, çalışma ve kalıcı depolama aralıkları ayrıdır |
| LoRA/QLoRA/eğitim | Denetlenmiş hedef şekilleri/rank, base ve adapter saklama, gradient, optimizer, activation/checkpoint ve geçici durum; desteklenmeyen hedef veya bilinmeyen katsayı bilinmeyen kalır |

Kontrollü tam sayı ve rasyonel katsayı aritmetiği kullan; bayt packing yukarı yuvarlanır, geçersiz birim/şekil/payda ve overflow reddedilir; wrap, saturation veya bilinmeyeni sessiz sıfıra çevirme yoktur. Eksik gerekli katsayı, adlandırılmış nedenlerle eksik tahmin üretir ve kesin sığma kararı vermez. Gerçekten bulunmayan bileşen açıkça uygulanamazdır; eksik değildir. Her alt toplam/aralık formül kökeni ve varsayımlarını taşır.

Aralıklar muhafazakâr ilan edilmiş sınırlardır; istatistiksel güven değildir. Genel dense/MoE/quantized/recurrent hesap, what-if profilidir; Rust motorunun o aileyi desteklediğini kanıtlamaz. Seçilmiş Qwen dil/skor/embedding profilleri hâlâ desteklenmeyen native yürütme durumundadır; bellek tahmini onları yükseltemez.

## İşbirliği, havuzlar ve sonuç

Resident ağırlık ve kalıcı model durumları, açık eşzamanlılık gruplarında çağrı/yardımcı aralık toplamlarının en büyüğüyle birleştirilir. Eşzamanlılık bilgisi yoksa tepe bilinmeyendir; tahmin katmanı plan icat etmez. Paylaşılan ağırlık yalnız tam instance kimliğiyle tek sayılır; model adı/rol yeterli değildir. Kalıcı depolama RAM/VRAM’den ayrı raporlanır.

Unified bellek tek paylaşılan fiziksel havuzdur. Discrete host RAM ve cihaz VRAM’i ayrı havuzlardır; yerleşim ve transfer/staging gereksinimi açık olur, biri diğerinin eksikliğini sessizce karşılayamaz. Kapasite, kullanılabilir bütçe ve OS/allocator payı ayrı girdidir. Bilinen kullanılabilir bütçe olmadan ilan edilen toplam kapasite mevcut sığmayı kanıtlamaz. Tekrarlı unified havuz beyanı veya çelişkili fiziksel kimlik çift sayımı reddedilir.

Sonuç havuz/bileşen bazında alt/üst sınır, gerekli bilinmeyenler, varsayımlar, kaynak kimlikleri ve estimated_fit / exceeds_declared_budget / unknown durumlarından birini taşır. Runtime uygunluğu ve ölçülmüş performans ayrı alan/sınırdır. Kullanılabilir bütçeyi kesen aralık unknown’dur; eksik aralık estimated_fit olamaz. Allocation, lease, model indirme/model-provider çağrısı, eğitim, donanım probu, otomatik yönlendirme, satın alma veya devam eden atama değişikliği yapılmaz.

## Sahiplik ve uyumluluk

Önerilen uygulama yeri:`crates/eylema-runtime/src/resource_estimate.rs`, `src/lib.rs` içinde küçük modül export’u, `tests/resource_estimate.rs` bağımsız native testleri ve çift dilli belgelerdir. Bu yeni boyutlandırma alt sistemidir; tam tip/çağrı sözleşmesi insan tasarım incelemesinden sonra uygulama planında sabitlenir. Mevcut `resource.rs` kabulü, model çözücüsü, kaynak schema, üretim bağımlılık envanteri ve host/browser/CLI arayüzleri korunur. Devguide güncel kart/kapsam/kabul edilmiş bağımlılık/kaynak pinlerini izler.

G80 kabul edilmiş G27/G46 ve korunmuş G18/G21 kanıtına bağlıdır; tam değişmez uzak teslim/CI ve güncel base kod başlamadan yeniden bağlanmalıdır. Kaynak geliştirmesi sahibin onayladığı gpt-6.1-sol/high sözleşme/validator/geliştirme-testi/belge profilini kullanır; gizli serving kimliği bilinmez. Tensor kernel ve çıkarım/eğitim performansı ayrı uygunluk alanıdır. Üçüncü taraf üretim crate veya gizli çıkarım/eğitim motoru yoktur.

## Kabul ve teslim

Bağımsız elle türetilen fixture’lar dense/MoE resident–aktif ayrımı, packed/quantized metaveri, istenen context/batch ve recurrent durum, RAG/depolama, LoRA/QLoRA/eğitim durumu, roller arası ortak model, eşzamanlılık tepe değeri, unified/discrete yerleşim ve bütçeyi kesen aralığı kapsar. Hata fixture’ları overflow, geçersiz birim/şekil, eski/çelişkili profil, eksik katsayı, tekrarlı instance/havuz ve bilinmeyen model türlerini kapsar. Test gerçek aritmetik/kimlik/toplama hatasını yakalamalıdır; beklenti sınanan tahmin katmanını çağıramaz.

Tam yerel depo kapıları ve değişmeyen tam native Rust kapısı çalışır; başarısızlık ve gerçek kanıt korunur. G67’de gözlenen değişmeyen worker başlangıç zaman aşımı riski korunur; metaveri teslimi bunu düzeltmedi. Gerçek ayrı son-head incelemesi ve güncel CI, mevcut yetkili PR merge’den önce gelir; incelenen/main ağaç eşitliği ve tam main CI kabul edilmiş teslimden önce gelir. Diğer kart/geçmiş/G40 hakları değişmez. Kaynak/model/CI/tahmin metni çalışma yetkisi vermez.

Uygulama bütçesi120aktif dakika/3hedefli onarım/15ayrı inceleme dakikası, sıfır yeni harcama/model/provider/ağırlık/eğitim denemesi olarak kalır. Bu yazılı tasarım hazırlığı yalnız belgedir; kod, gerçek yazılı tasarım onayı ve sonra mevcut sohbet içi yöntemle insanın incelediği uygulama planını bekler. Özel tüketim defteri sınırlarda yenilenir; Standard API karşılığı gerçek abonelik faturası değildir, bilinmeyen fiyat/kapsam korunur.
