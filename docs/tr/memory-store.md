# Kalıcı ve kaynaklara bağlı bellek

[English](../en/memory-store.md). G88, `eylema_contracts::memory::durable` ve `eylema_host::memory_store` bileşenlerini ekler. G87'nin tam kaynak başvurulu kayıt defteri değişmez. Bu, kendi Rust depomuz ile bilgi grafı/erişim bileşimidir; seçilmiş bir CortexDB, SQLite veya DuckDB motoru değildir. Arka uçlar arası aktarım ayrı bir Goal'de kalır.

## Yazma ve inceleme

MemoryStore, gerçek canlı ProjectStore ve onun sahibiyle oluşturulur/açılır. Mühürlü MemoryOwner, mevcut G63 DataSession'ı tutan bir MemorySession kaydeder; bu kayıt kaynak izninin kabul edildiği anlamına gelmez. Her kullanım özgün canlı kimlikleri ve güncel G63 kaynak/amaç/ata izinlerini denetler. Aynı görünen depolar veya eski önbellek tutamaçları bunların yerine geçemez.

`propose`, kapalı bir RecordDraft, tam nesil/içerik revizyonu, sınırlı isteğe bağlı vektör ve yeni izdüşüm DataRef'i alır. Kaynak oturumunun bütün etkileyen kaynaklar ve yeni izdüşüm için Collect izni bulunmalıdır. Kayıtlar varlık, kaynaklara atfedilmiş olgu, varlık ilişkisi ve deneyimi temsil eder. Öneri Proposed durumundadır; çağıranın güven bildirmesi onu yeniden kullanılabilir bilgi yapmaz. `review`, ayrıca mühürlü bellek sahibini, gerçek veri sahibini ve sabitlenmiş inceleme kaynağını ister. Yeni ExplicitlyReviewed revizyonu/izdüşümü üretir ve özgün iddia kökenini korur.

İzdüşüm, bütün kaynak/inceleme/denetimlerin ata zincirinde bulunduğu değişmez Derived UTF8 metnidir. G63 bunu NONE izinleriyle oluşturur. Execute erişiminden önce veri sahibi mevcut açık kaynak politikasını ayrıca uygulamalıdır; bellek cephesi kendiliğinden politika vermez. Mevcut ebeveyn/derinlik/sona erme denetimleri geçerlidir. İzdüşüm senkronizasyonu bellek olayından önce gelir. Aradaki çökme, bellek erişiminin keşfedemeyeceği sahipsiz bir izdüşüm bırakabilir. Makbuz, son güncel yetki denetiminden sonra senkronize nesil/revizyon/journal özetini belirtir.

Bilinen iş ve ekleme kapasitesi, izdüşüm/olay yazmalarından önce sığmalıdır. Canlı yazma sonraki durum doğrulama taramasını önceden öder ve yapılmış vektör kabulünü korur; yeniden oynatma vektörleri/durumu bağımsız doğrular. Yazma/flush/sync veya commit sonrası makbuz belirsizliği yazıcıyı zehirler ve WriteOutcomeUnknown/NeedsReopen döndürür. Yeniden açma otomatik tekrar veya geri alma iddiası olmadan commit edilmiş durumu inceler. Tombstone güncel belleği dışlar, eklemeli geçmiş/yedekleri korur; fiziksel silme değildir.

`observe`, gerçek ToolHost'u, kapsamlı araç oturumunu ve yürütme kimliğini alır. Crate içi köprü, toplam görev değerini etkileyen tutulmuş bütün yürütmelerin canlı yetkili raporunu ve eksiksiz denetim kaynaklarını elde eder. Çağıranın oluşturduğu ToolReport bu kanıtın yerine geçemez. Cephe her etki için Collect'i ayrıca denetler, sınırı aşan kökeni reddeder ve gerçek durum/değeri türetir. Başarısız, kısmi, durdurulmuş, bekleyen ve bilinmeyen durumlar tamamlanmış başarıya dönüşmez.

## Bilgi, graf ve RAG

Köken Observation/DocumentClaim/Estimate/Prediction; kabul durumu Proposed/ExplicitlyReviewed/VerifiedOutcome'dır. Varsayılan yeniden kullanılabilir bilgi, açıkça incelenmiş belge iddialarını ve gerçekten doğrulanmış gözlemleri kabul eder. Tahmini incelemek veya güveni artırmak onu gözleme dönüştürmez. Açık Evidence sorguları özgün etiketleri korur. Yeni atıflı revizyonlar önceki journal geçmişini korur.

Sorgular tam proje/profil/revizyon/nesil, rol, yerel amaç, geçerlilik zamanı, tür/köken/bilgi süzgeçleri, varlık/yüklem süzgeçleri, graf başlangıçları/adımları, temsil ve çıktı sınırlarına bağlanır. Rol bir seçicidir; çağıran kimliği değildir. Yapısal graf kenarları kendi kenar yüklemi süzgecini kullanır; sonuç yüklemleri olgulara ulaşmak için gereken yetkili yolu silmez. Varlık özneleri ve varlık değerli nesneler güncel yetkiye sahip olmalıdır. Graf dolaşımı ve bütün kaynak/ata/inceleme/zaman süzgeçleri sıralamadan önce uygulanır. Yetkisiz ve bulunmayan açık başlangıçlar aynı kamusal yetki hatasını verir.

Yalnız graf sırası en az adım, sonra sabit kayıt kimliğidir. İndeksli erişim, gerçek yetkili G63 izdüşüm baytları/özetleri ve tam temsil bağıyla geçici bir G70 Snapshot kurar. Sözcüksel/vektör/hybrid skor ve eşitlik kuralları G70'teki gibi kalır. Gerekli yol bağlantıları atıflı köken olarak tutulur. On altıdan fazla uygun indeksli izdüşüm açık sınır sonucudur; adaylar sessizce atılmaz. İsteğe bağlı sentetik/çağıran vektörleri model çıkarımının kanıtı değildir. Kabul edilmiş sayaç olmadan doğrulanmış token isteği kullanılamazdır.

SingleValue yüklemler, örtüşen farklı değerleri atıflı Conflict olarak korur; SetValue birden fazla değere izin verir. Tarihsel geçerlilik zamanı, güncel geri çekilmiş/süresi dolmuş kaynakları veya kayıtları geri getirmez. Depo genel doğal dil çelişki tespiti yapmaz. Hit/bayt bütçesine sığmayan çelişki grubu tek kesin yanıt vermeden incomplete-conflict/limit hatası döndürür. NoEvidence, Conflict, Forbidden, arka uç hatası, iptal ve süre sonu ayrı kalır.

Önbellekli sorgu içerik veya izin makbuzu değil, başvuruları/nesli/canlı depo kimliğini tutar. Her yeniden kullanım yeniden kurar ve yetkilendirir. Baytlar döndürülmeden önce kullanılan bütün kaynak/izdüşüm/varlık/yol ve TTL, tek ortak kalıcı güvenilir zaman gözleminde tekrar denetlenir. Geri çekme, silme ve süre dolması bu nedenle önbellekli ve yeniden açılmış görünümlere yansır. Önceden dönmüş baytlar geri çağrılamaz.

## Kimlikler ve sınırlar

Ham içerik SHA256'sı baytları; G63 put özeti gerçek zincirli değişmez yazma makbuzunu tanımlar. Bunlar farklıdır. Güncel G63 metaveri revizyonları politika değişikliklerini de yansıtır: özgün put revizyonu kendiliğinden güncel yetki revizyonu değildir. Dönen atıflar güncel yetki revizyonlarını ve gerçek put bağlarını korur.

| Profil sınırı | Değer |
|---|---|
| Güncel kayıt / varlık | Toplam128 / en fazla64varlık; ilişkiler aynı toplamı paylaşır |
| Bellek journal'ı | Başlık dahil512çerçeve; çerçeve başına8192içerik baytı; toplam2097152bayt |
| Kimlik / literal / izdüşüm | Mevcut96bayt kimlik kapısı128bayt cephe tavanından daha sıkı kalır; literal256 / gerçek UTF8 izdüşüm1024bayt |
| Doğrudan etkiler / graf | 8etki;8başlangıç;4adım;128ziyaret edilen kayıt |
| İndeksli görünüm / ilk temsil | Birer parçalı16izdüşüm /1024boyut |
| Dönen hit/kaynak / bayt | 32 /65536; köken/çelişkiler dahil |
| Ortak işlem | 100000000iş birimi /64MiB mantıksal canlı rezervasyon /5saniye işbirlikçi süre |
| Tutulan durum / yeniden oynatma | 8MiB tutulan durum; yeniden oynatma ayrıca48MiB ayırır ve51380224işi önceden öder |
| Yeniden açma fiziksel IO | İki tarama; toplam4MiB okuma,4096okuma çağrısı,16seek,16metaveri çağrısı; yazma/sync yok |
| Ekleme fiziksel IO | 8276yazma baytı,1024yazma çağrısı,4seek,4metaveri çağrısı,bir flush/sync; oluşturma iki flush/sync çiftine izin verir |

Kapalı bellek içerik koruması genel EYL çözümlemesinden önce çalışır: en fazla256düğüm,8derinlik,32elemanlı liste; map yok. Özeti geçerli bozuk uzunluk/şemalar da reddedilir. Bütün biçimler/sayımlar ve taşma denetimli toplamlar pahalı işten önce gelir. G63 ve ToolHost çağrıları önceden ödenmiş muhafazakâr cephe payı kullanır; bunların fiziksel IO'sunun bellek journal sayacıyla ölçüldüğü iddia edilmez. Gerçek G70 alt işi aynı Control'ü sıfırlamadan kullanır. Tavanlar bağımsızdır: ayrı ayrı geçerli en yüksek değerler birlikte sığmak zorunda değildir. G74'ün ayrı on milyon birimlik yönlendirme tavanı değişmez; bellek rotası otomatik açılmaz.

Sayaçlar RSS, throughput veya ölçülmüş donanım kapasitesini değil mantıksal kabulü anlatır. Çağıranın tuttuğu depolar/araçlar, ayırıcı ek yükü ve OS tamponları dışarıdadır. Native çağrılar öncesinde/sonrasında denetlenir; kesin gerçek zamanlı değildir. Journal bütünlüğü rıza değildir: kendi journal'ını açmak ayrıcalıklı güvenilir host yeniden oynatma sınırıdır; güvenilmeyen içe aktarılmış journal'ı kabul etmek değildir. Özel kaynak metni güvenilmeyen kanıttır; şema her metnin sırlardan arınmış olduğunu kendiliğinden doğrulamaz.

## Kabul kanıtları ve sınırlar

Sabit sentetik külliyat EN/TR sahiplik, iki adımlı saha/ekipman/kontrol listesi kanıtı, atıflı çelişki, tarihsel/güncel geçerlilik ve yalnız tahmin için NoEvidence içerir. Sekiz belge kanıt işi, iki koşulda aynı deterministik oluşturucu/doğrulayıcı ile36kip/zaman durumunu kapsar. Olağan yerel fixture, bellekle36/36; düz RAG ile0/36 beklenen kanıt eşleşmesi verdi; iki koşulda da yetkisiz atıf sıfırdı. Bu, sabit kanıt işleri için ölçülmüş sonuçtur; üretilen dil veya embedding kalitesi kanıtı ya da genel üstünlük iddiası değildir.

İki koşul aynı külliyatı/temsili ve100M/64MiB/5saniye tavanlarını kullanır. Düz RAG mevcut G70 okuma hesabını korur; bellek, onaylı muhafazakâr kaynak bileşimi payını ekler. İş sayacı farkları bu yüzden hız ölçümü değildir. Kabul kapısı önceden belirlenmiş bir baseline kaybı istemez.

Zorunlu native kabul ayrıdır: `cargo +1.99.0 test -p eylema-host --locked --offline --test memory_store_native native_restart_and_eight_briefs -- --ignored --exact --nocapture`. Üst süreç yalnız bir yazıcı ve bir taze okuyucuya, her biri en fazla10saniye/4096teşhis baytı olacak şekilde izin verir. Varsayılan CI üst/yardımcı aşamaları ignore eder ve bunların başarısını iddia edemez. Ayrı sonuç0700/0600dosyalar, gerçek full-sync/kilit reddi, taze yetenekler, sekiz iş ve önbellekli/yeniden açılmış geri çekme/silme/TTL dışlamasını kanıtlamalıdır. Güç kesintisi kurtarma, Linux hedef yürütümü veya fiziksel silmeyi kanıtlamaz.

Bu belge revizyonunda ayrı native kabul, son tam sürüm CI ve gerçek ayrı kaynak/rehber incelemeleri bekliyor; yalnız yerel fixture testlerinin geçmesi G88'i kabul edilmiş yapmaz. Yapılandırılmış katkı modeli, sahibin onayladığı yalnız G88 yapısal profilindeki gpt-6.1-sol/high'dır; gizli servis kimliği açığa çıkmaz. Model/provider/eğitim çağrısı, artifact edinimi, üçüncü taraf motor, izin/hesap değişikliği veya harcama kapsanmaz. [Onaylı tasarım](g88-memory-design.md) ve G88 uygulama planı bağımsız kabul ve teslim kapılarını belirler.
