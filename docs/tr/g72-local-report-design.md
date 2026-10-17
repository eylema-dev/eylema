# G72 özel yerel rapor işi tasarımı

Durum: bakımcı incelemesi için yazılı tasarım. G72 ürün kodu, artifact erişimi veya model yürütümü başlamadı. [English canonical](../en/g72-local-report-design.md). [Issue79](https://github.com/eyleme-dev/eyleme/issues/79), önerilen G72/r4.

## 1. Sonuç, kanıt ve kapsam

Kendi Rust embedding, planlama ve karar modellerimiz, tipli işlemler, olgu/atıf doğrulaması, tek sınırlı saf-adım onarımı, etkili insan durdurması ve kalıcı yeniden açma üzerinden gerçek, projeye özel iki dilli kaynak bağlı rapor tamamlanmalıdır. Modeller kabul edilen kanıt/seçim yolunu etkilemelidir; sayaç gösterimi veya model çıktılarını kullanmayan rapor şablonu kabul sayılmaz. Rapor olguları kaynaklı belge iddiaları olarak kalır; kalibre edilmiş model doğrusu değildir.

Kaynak tabanı `cbad05aa256541858c595ba7f33fc1068709fb55`, rehber tabanı `0870a4dbe437fad84034ce16b5b62cd86cc46a45`. Değişmeyen dokuz bağımlılık G30/G57/G68/G70/G54/G50/G63/G73/G88 kabul makbuzlarına sahiptir. Kanıtları ayrıdır: G30 kısa doğrulanmamış plan önerileri, G57 kalibre edilmemiş ilgi seçimleri, G68 sayısal embedding, G70/G88 sınırlı erişim/bellek, G54 iki sayaç aracı, G73 açık cihaz sahibi amaçları sağlar. Hiçbiri bu rapor işini zaten kanıtlamaz.

Bakımcı uygulama katmanında birleştirmeyi ve bu tasarım/uygunluk değerlendirmesini hazırlamayı, önerilen first-party Cargo.lock ve G72 rehber kapsamıyla onayladı. Bu karar henüz bu yazılı tasarımı, aşağıdaki ek rapor-amacı yolunu, uygulama planını veya model/artifact denemesini onaylamaz. Özgün120aktif dakika/3hedefli onarım/900inceleme saniyesi hazırlığı içerir; sıfırlanmaz. Mevcut sohbet içi uygulama ve koşullu merge yöntemi korunur.

## 2. Mimari ve sorumluluk

Native CLI içindeki uygulamaya ait `report` modülü mevcut kütüphaneleri birleştirir. Aynı binary'nin sabit worker'ı modelleri yürütür; proje sahibi/oturum veya etki yetkisi taşımaz. Denetleyici ProjectStore,MemoryStore,ToolHost,gerçek insan denetleyicisi,doğrulanmış rapor izinleri ve durum geçişlerine sahiptir. Listener,tarayıcı,harici provider,eğitim motoru,yeni crate veya üçüncü taraf bağımlılık eklenmez.

| Bileşen | Sorumluluk ve sınır |
|---|---|
| ReportNeed | Kapalı proje/iş/generation,iki zorunlu EN/TR yer/alan bölümü ve bir isteğe bağlı NoEvidence denetimi,yetkili kaynak pinleri,atanmış model/profil kimlikleri,kaynak inceleme kabulü,çıktı ID'si ve bütçeler; keyfi araç/süreç/provider alanı yok |
| Kaynak araçları | Güncel izinli yapılandırılmış belgeleri okur; kaynak özet/sürümleri ve yerel Execute/Collect ata izinlerini korur; otomatik izin veya inceleme terfisi yok |
| Model worker'ı | Gerçek G68 embedding,G30 Planning önerisi ve G57 kararı; sabit tipli protokol,tam artifact kimlikleri,araç yürütümü yok |
| Rapor araçları | Uygulamaya ait kapalı Rust PrepareReport/VerifyReport/CommitVerifiedReport/ReopenReport işlemleri; runtime isteği callback veya handler kuramaz |
| G54 kontrol adımı | Gerçek kaynak bağlı AddTaskValue/ReadTaskValue kabulü,insan onayı/müdahalesi ve gözlenen kanıt; hâlâ yalnız mevcut iki handler |
| ReportPermit | Doğrulanmış bayt/özet,Need/generation/kaynak pinleri,gerçek tamamlanmış kontrol adımı ve insan kabulünü bağlayan; serileştirilemeyen,kopyalanamayan uygulama tutamacı; model/CLI JSON'undan kurulmaz |

Rapor işlemleri G63/G88 ve mevcut denetim API'lerini kullanan yeni uygulama tüketicileridir; **G54 ToolRegistry içine yeni işlem eklenmez**. Kontrol adımındaki değer doğrulanmış olgu sayısıdır; raporun olgusal cevabı değildir. Tamamlanmış kontrol adımı tek başına ReportPermit veya JobCompleted oluşturamaz. Contracts/runtime handler veya genel callback genişlemesi önerilmiyor.

Yaşam döngüsü: Validated -> SourceReviewed -> Computing -> EvidenceSelected -> Prepared -> AwaitingApproval -> CheckpointCompleted -> ReportCommitted -> Completed. NoEvidence,Abstained,Denied,Unsupported,Failed,Stopped,Partial ve OutcomeUnknown ayrı terminal sonuçlardır. Doğrulanmış kontrol adımından sonraki hata bu etkiyi korur; geri alma veya tamamlanmış rapor gibi gösterilemez.

## 3. Gerçek model ve RAG akışı

Yalnız kabul edilmiş G30 Qwen3-0.6B CPU profili (`abc9669adb66de6ca55e86f5a6793e6b6f2a694eee260f63dcc7120794b7fcc5`),G57 reranker CPU profili (`c47cc3f8008e4735ab43acfde124b884cab99f9206913cbecfc79ab06e2d0cdc`) ve G68 embedding profili (`ca3cde459437692c9ae877b66cf4d15bab58c0852ec2f0fc7dc08e7003fdc71f`) kullanılır. Mevcut tam artifact/tokenizer/ön işleme/düzen/hak kimlikleri eşleşmelidir; bu tasarım alternatif profil veya yeni indirme kabul etmez.

1. Sınırlı EN/TR yapılandırılmış kaynak belgeleri güncel G63 yetkisiyle okunur. Ürün ayrıştırıcısı language/site/owner/checklist alanlarını kabul eder; UTF8,kapalı şekil ve boyutları doğrular. Testteki ad/cevaplar ürün sabiti değildir. Açık kaynak-inceleme kabulü Need ve gerçek insan başlatmasına bağlanır; kimlik doğrulaması tek başına iddiaları doğrulanmış yapmaz. İnceleme yoksa Proposed korunur; yeniden kullanılabilir bilgiye terfi yerine NeedsSourceReview döner.
2. G88 varlıkları ve kaynak bağlı,incelenmiş sahip/kontrol-listesi iddiaları kurulur. Salt okunur host projection-preview yöntemi G88'in saklayacağı tam sınırlı baytları gösterir; yetki vermez. Bu baytlar G68 belge ön işlemesiyle embed edilir; gerçek toplama sonrası saklanan bayt/özet önizlemeyle karşılaştırılır,uyuşmazlık reddedilir. Farklı projeksiyon metnine aitmiş gibi belge vektörü kullanılmaz.
3. Gerçek sorgu kabul edilmiş G68 önekiyle embed edilir. G88/G70 bu vektörleri ve gerçek kaynak bağlı projeksiyonları kullanır. Dil başına ayrı sınırlı bellek/erişim işlemleri çalışır;commit/sunum öncesi birleşik pinler tek ortak güvenilir zaman gözleminde yeniden yetkilendirilir.Yetkili derlem üzerinde,candidate yürütümünden önce sabitlenen sınırlı sonuç kümesiyle sıralanır. Yanlış konu veya eksik alan rapor doğrulamasını karşılayamaz. Önceden tanımlı,sınırlı eksik-alan yeniden sorgusu tek onarım hakkını tüketebilir; sentetik vektör fallback'i veya sessiz top-k artışı yoktur.
4. Kısa istekle gerçek G30 Planning çalışır. Geçerli kısa PlanProposal,Need ve bulunan kanıtla birlikte gerçek G57 sorusuna eklenir; makbuz/girdi özeti bu bağı korur. Çalıştırılabilir eylem sözdizimi değildir. Karar yalnız listelenmiş erişilmiş aday paketlerden seçer veya çekimser kalır. Mevcut750000taban/100000fark,eşitlik/eksik/desteklenmeyen kuralları ve kalibre edilmemiş durum değişmez. Fixture'ı geçirmek için seçim zorlanmaz.
5. Seçilen gerçek kaynak değerlerinden rapor baytları hazırlanır. Gerekli konu/alan kapsamı,atıflı çelişkiler,tam güncel atıf/pin ve kaynak hakları modelden bağımsız doğrulanır. Yalnız doğrulanmış baytlar ve taze açık insan onayı kontrol/commit adımına geçebilir.

G30 context'i prompt ve üretimi birlikte içerir: toplam en fazla64token;8üretim tokenı ayrıldığında render edilmiş prompt56tokena sığmalıdır. Kesme veya G30/G57/G68 sayısal kod değişikliği yoktur. Sabit iş kabul edilmiş profillere sığmazsa olgusal ölçüt düşürmeden veya context'i sessizce büyütmeden somut profil/kalite/kaynak engeli bildirilir. Sabit yapılandırılmış EN/TR işlerinden geniş dil kalitesi,keyfi düz metin çıkarımı veya genel otonom planlama çıkarılmaz.

## 4. Gerçek insan denetimi ve durdurma

Önerilen yeni `HumanPurpose::LocalReport`, kişinin `state:report` üzerinde tek özel raporu en fazla beş dakika için başlatıp/kaynaklarını inceleyip,onaylayıp durdurduğunu belirtir. Bu **önerilen ek** kaynak kapsamıdır: `crates/eylema-platform/src/human.rs`; yalnız amaç/kaynak/açıklama ve amaca bağlı regresyon durumları. Mevcut LocalCli/AcceptanceProof amaçları,native ABI/selector ve doğrulama uygulaması değişmez. Uygulamadan önce yazılı tasarım kararı bu yolu açıkça onaylamalıdır.

`enroll_verified_human` fixture'ları yerine görev/Need bağlı host sarmalayıcıları kullanılır:

- request_report_controller(owner,grant,task,need_digest,ttl),kapalı ReportControlRequest döndürür. Doğrulanan challenge tam host/grant/nonce/task/Need özeti ve süreyi içerir. Genel,bağsız kayıt LocalReport kabul edemez.
- enroll_report_controller,gerçek eşleşen DeviceOwnerProof'u kapalı ReportController'a tüketir. Yeniden kullanılan,yanlış amaç/görev/Need/örnek veya süresi dolmuş kanıt reddedilir. Kanıt,erişim bilgisi veya insan oturumu serileştirilmez.
- check_report_controller,güncel örnek/görev/Need,değişmez kapsamlı haklar,politika ve kalan lease'i doğrular. İptal denetimi kaynak gövdesi okumaz; geri çekme durdurmayı kapatamaz.
- Rapor approve/cancel/revoke sarmalayıcıları mevcut ToolHost kaydını ve gerçek bekleyen-iptal takibini korur; ham değiştirilebilir DurableHost açılmaz.

Terminal stdin denetleyicide,worker stdin/stdout'u özel sınırlı pipe'larda kalır. Model/kaynak metni approve/stop komutuna dönüşemez. Commit öncesi kişi güvenli kaçışlı doğrulanmış olgu/atıfları görüp ekrandaki hazırlanmış özet/generation'ı onaylar. Model bayrağı veya kayıtlı istek bu kabulü oluşturamaz. Gerçek G54 onayı somut kontrol etkisini kapsar; ayrı uygulama onayı ayrıca gerçek hazırlanmış raporu bağlar. İkisi de zorunludur.

Durdurma bütün kayıtlı CpuControl'leri iptal eder,yeni dispatch'i keser ve son rapor bırakmaz. Parent monotonik son süre IPC boyunca esastır; child'ın göreli süresi bunu sıfırlayamaz. Lease kaybı,kaynak geri çekme,süre ve ret de sonraki işi durdurur. İşbirlikçi iptal hard real-time kesme değildir. Tolerans süresi aşılırsa yalnız sahip olunan worker sonlandırılıp beklenir,belirsizlik kaydedilir; kill'den işbirlikçi başarı çıkarılmaz.

## 5. Gizlilik, commit ve yeniden açma

Kaynaklar,projeksiyonlar,embedding,kısa tipli öneri,karar girdileri,hazırlanmış/son rapor,maliyet ve inceleme/denetim makbuzları projeye özel türevlerdir. Girdi/makbuz özetleri ve gerekli tipli öneriler tutulur; ham sohbet,özel muhakeme izi,token dökümü,erişim bilgisi veya worker stderr'i tutulmaz. Geçerli keyfi metin sırdan arınmış sayılmaz. Train/ExternalDisclosure/EcosystemExport hakkı oluşturulmaz.

Her model dispatch'i güncel Execute,her türev yazma Collect denetler; seçim,etki kabulü,commit ve son sunum tam güncel ata izinlerini,aktör/insan lease'ini,kaynak özet/sürüm ve TTL'yi yeniden denetler. Parent döngüsü çalışan worker'ı iptal edebilir; kopyalanmış baytlar geri çağrılamaz. Bütün etkiler G63'ün mevcut16doğrudan ata/32derinlik ve G88'in8etki/32kaynak sınırına sığmalıdır; aksi durumda yazmadan önce reddedilir,soy bağı kesilmez.

Kontrol adımı öncesi doğrulanmış hazırlanmış rapor ve tam atalarından türetilmiş kanonik unsigned olgu-sayısı girdisi kalıcılaştırılır. G54 eylemi bu gerçek kaynağa bağlanır. Gerçek gözlenen tamamlanmış ToolReport ve G88 gözlenen episode alınır; çağıranın raporu yerine geçemez.Tam görev/execution/action,kanonik sayı girdisi put pini,uygulanan sayı ve hazırlanmış raporun bütün ataları eşleştirilir;başka execution kanıtı geçerli olsa da InvalidWitness olur.Seçimde bilgi generation'ı sabitlenir;yalnız açıkça beklenen gözlenen-episode eklemesi,özgün olgu sürüm/projeksiyon özetlerinin değişmediği doğrulanınca bunu ilerletebilir.Başka değişiklik sessiz yeniden bağlama yerine RevisionConflict olur. Sonra ReportPermit kurulup son rapor,tam hazırlanmış baytlar,onay/denetim,model makbuzları ve gözlenen episode soy bağıyla toplanır. Senkronizasyon sonrası makbuz dönmeden güncel denetimler tekrarlanır. Belirsiz yazma/makbuz yetkisi veya kontrol ile son collect arasındaki açıklık,etki ve yetim kayıtları koruyarak OutcomeUnknown/Partial üretir.

Kalıcı durum/makbuzlar yeni ortak journal formatı yerine sınırlı değişmez G63 kayıtlarını kullanır. Yeni süreç gerçek özel dosyaları taze sahip/oturumla açar,pinli rapor/makbuzları doğrular,bütün kaynaklara güncel izin uygular. Tamamlanmış rapor incelenebilir; geri çekilmiş/süresi dolmuş çıktı okunamaz. G54 kurtarılmış tipli görevleri sürdüremez: eski komut geçmişi üzerinde ToolHost yeniden kurulmaz,etkiler otomatik tekrarlanmaz. Kesilmiş işler ayrı uzlaşma olmadan Stopped/Partial/OutcomeUnknown kalır. Yeniden açma exactly-once veya fiziksel silme iddiası değildir.

## 6. Sabit kabul ve kaynak tavanları

Tam belge metinleri,prompt'lar,aday kurma,adım token/bayt/iş tahminleri ve beklenen atıflar model yürütümünden önce uygulama planında dondurulur. Olgusal oracle bağımsız sabit değerleri kullanır: EN Site North/owner Mira/checklist C7;TR Güney/sorumlu Ece/liste C9;ilgisiz ve çapraz-proje yanıltıcıları. Ürün değerleri kaynaktan okur. Sabit native Need iki zorunlu iki-dilli bölüm ve isteğe bağlı bilinmeyen-yer denetimi içerir. O bölümde NoEvidence açıktır; bütün zorunlu isteğin eksik olması kökte NoEvidence ve tamamlanmış rapor olmaması demektir. Beklenen cevap model sorgusuna verilmez; ürün kodu onu test sabitinden kopyalamaz.

| Zorunlu durum | Kanıt |
|---|---|
| Gerçek iki dilli iş | Kendi gerçek embedding/planlama/kararı seçimi etkiler;iki doğru kaynak bağlı bölüm/dört olgu ve tam güncel atıflar;izinsiz atıf sıfır;isteğe bağlı bilinmeyen-yer denetiminde olgu/atıf olmadan gerçek NoEvidence |
| Ret/desteklenmeyen | Yabancı proje/amaç,eksik inceleme,eski kimlik ve desteklenmeyen model;yetkisiz gövde/model/etki işinden önce reddedilir;fallback yok |
| Kanıt yok/çelişki | Gerçek erişimde uygun istenen olgu yoktur veya çelişkili iddia vardır;açık NoEvidence/Conflict,uydurulmuş tek sonuç yok |
| Sınırlı onarım | Önceden tanımlı tek saf Prepare/Verify veya eksik-alan onarımı;ikinci hata durur.Belirsiz yazma/etki tekrarlanmaz,sayaç sıfırlanmaz.Sentetik enjeksiyon ve gerçek model kanıtı ayrı etiketlenir |
| İnsan durdurması | Taze gerçek kişi gerçek planlama makbuzu sonrası ve son etkilerden önce durdurur;sonraki karar/commit/çıktı yok,önceki kontrol yazmaları korunur.Sahte,süresi dolmuş veya yabancı denetleyici başka işi durduramaz/onaylayamaz |
| Taze yeniden açma | Yeni süreç,güncel haklar,aynı son bayt/özet/soy bağı;eski tutamak,geri çekilmiş/süresi dolmuş atıf ve belirsiz iş,otomatik sürdürme olmadan reddedilir |
| Bağ denetimleri | Plan metnini karar girdisinden kaldırma,gerçek embedding/projeksiyon girdisini değiştirme,sahte ToolReport veya verifier/stop atlama ilgili regresyonu düşürmelidir |

Sonraki planda genişletilmeden somutlaştırılacak önerilen tavanlar: bir iki-dilli pozitif model işi,bir durdurulmuş iş;her biri için en fazla bir worker ve bir taze okuyucu child(toplam üç child);otomatik tekrar veya yeni oracle yok. Pozitif kök en fazla2.5e12CPU işi;durdurma kökü1e11;toplam2.6e12. Artifact kabulü ve başarısız/onarım işleri sayılır;tüketimi bilinmeyen başarısız worker'ın tam tahsisi tüketilmiş sayılır. Pozitif model çağrıları en fazla9embedding,2planning ve2decision batch(her biri<=2soru×2seçenek);durdurma işi en fazla1planning ekler.Bağımsız azami değerler birlikte sığma anlamına gelmez;karşılanamayan adım önceden reddedilir.Başarı bütün tavanları harcamayı gerektirmemelidir.Tasarım tek başına çağrı yetkilendirmez.

İnsan kaydı mevcut<=60saniye doğrulama ve doğrulamayı içeren<=300saniye lease'i;mevcut120saniye kayıt yaşı/15saniye kanıt tazeliğini korur.Kök<=300saniye,compute worker<=180saniye ve daima kalan lease/kök bütçesi içinde;yenileme/sıfırlama yok.Gerçek donanım buna sığmazsa somut karar için durulur.IPC<=256kapalı frame,<=8192bayt/frame,<=1MiBtoplam;map yok,derinlik<=8,list<=32,tahsis öncesi checked boyutlar.Worker teşhisi<=16KiBtemizlenmiş sabit kategoriler.Native okuyucu<=10saniye/4096teşhis baytı.Gerçek kişi katılımı ve tam süreç/model deneme tahsisi ancak çalışır artifact ve yazılı uygulama planı hazırken istenir.

Aynı anda yalnız bir modelin ağırlık/tokenizer/workspace'i yüklenir;sonraki profil öncesi bırakılır.Worker mantıksal resident<=2.5GiB;denetleyici ayrı<=64MiBve sınırlıIPCayırır,toplam<=3GiB.Bunlar kabul tavanıdır;ölçülmüş RSS,sığma veya throughput değildir.Bellek/erişim işlemleri ayrı100M/64MiB/5saniye tavanını korur;G74'ün10Myönlendirme bütçesi ve dispatch'i değişmez.Kök ayrıca facade/denetim işini4e9birim ve32bellek işlemiyle sınırlar;her işlem daha küçük kabul edilmiş tavanını korur.CPU skaler işi ile facade işi ayrı izlenir,farklı birimler aynı sayılmaz.Ölçülürse gerçek token/iş/süre/tepe/RSSörnekleri raporlanır;ölçüm olmadan enerji/parasal donanım maliyeti bilinmez.Sıfır provider çağrısı hariciAPIücreti olmaması demektir,sıfır yerel maliyet değildir;Codex geliştirme tüketimi ayrı özel defterdedir.

## 7. Dosya haritası, uygunluk ve teslim

Hazırlık yalnız bu EN/TR tasarımı ve incelenmiş çeviri parmak izlerini değiştirir.Önerilen üretim yolları:

- `apps/eylema/src/{main.rs,cli.rs,report.rs}`,yeni `report/{types,source,models,worker,control,verify,storage,wire}.rs`,app Cargo.toml ve rapor unit/native testleri.
- `crates/eylema-host/src/{tool.rs,human.rs}`,yeni `tool/report_control.rs` ve görev bağlı gerçek denetleyici host testleri;mevcut memory_store modüllerinde projection-preview yöntemi/testleri.
- Yalnız `crates/eylema-platform/src/human.rs`:yeni amaç/testler için,açık kapsam onayına bağlı.NativeABI veya model/kernel değişikliği yok.
- Mevcut first-party model bağı için kök Cargo.lock;kök Cargo.toml,yeni crate,contracts/runtime handler,G40/provider sınırları,workflow/checker veya politika değişikliği yok.
- Onaylı hazırlık önerisindeki yalnız G72 rehber plan/kart/profil/EN/TR aksiyon planı/core-pin/parmak izi yolları.Diğer67kart,özgün bağımlılık/sıra/bütçe/geçmiş/önceki profiller ve core dışı pinler değişmez.

Önerilen yapılandırılmışgpt-6.1-sol/high bağı yalnız G72 yapısal uygulama/model-SDKbirleşimi,sınırlı doğrulama/IPC/kaynak soy bağı,mevcut insan-amacı köprüsüne dar ek ve gerçek ayrı incelemeyi kapsar.Sayısal/çıkarım/eğitim/performans/global-SOTAuygunluğunu genişletmez,gizli servis kimliğini açığa çıkarmaz.Gerçek bakımcı kabulü hâlâ bekliyor;G88uygunluğu otomatik taşınmaz.

Yazılı tasarım/profil/kapsam incelemesinden sonra tam dosyalar,dondurulmuş referanslar,bütçeler,indirmesiz önbellek artifact kimlikleri ve süreç/fiziksel kabul koşullarıyla uygulama planı hazırlanır.O plan yetkilendirilmeden artifact erişimi,ürün kodu veya runtime çağrısı yoktur.Ardından anlamlı regresyonlar,gerçekM4kabulü,tam yerelPython/Rust/fmt/Clippy/derleme/Wasm/belge denetimleri,gerçek ayrı son-sürüm kaynak/rehber incelemeleri,güncelCI,yetkili merge ve eşitmainağaçları/commit ilişkisi/tammainCIgerekir.Kaynak/statik denetimler model/kişi/RSSkanıtı değildir.Geri alma özel kayıtları ve doğrulanmış etkileri koruyan sınırlı incelenmiş revertPR'ıdır.
