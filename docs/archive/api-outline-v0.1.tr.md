# Eylema — API sözleşmesi ve insan denetimi — v0.1

> Eylema'nın güncel esas kaynağı [English API reference](../en/api-reference.md), tam çevirisi [Türkçe API referansı](../tr/api-reference.md) olarak taşındı. Kaynak revizyonu: `0.1.0`. Aşağıdaki metin önceki taslaktır; güncel işlem kataloğu ve HITL/HOTL/HIC tanımları için yeni referansları kullanın.

Bu belge, kendi Rust bileşenlerimizle geliştirilecek platformun API ve dokümantasyon tasarımını tanımlar. Buradaki işlem adları, alanlar ve örnekler öneridir; uygulanmış, çağrılmış veya üretimde doğrulanmış bir API’yi temsil etmez. Amaç, yerel eylemcilerin, büyük modellerin ve insanların aynı davranış sözleşmesini kullanmasını sağlamaktır. Arayüzler farklı olabilir; yetki, veri anlamı ve işlem sonuçları aynı kaynaktan türetilmelidir.

## 1. Tek sözleşme, farklı kullanıcı yüzeyleri

Her işlem, bize ait tipli Rust tanımlarında tek kimlikle tanımlanır. Girdi, çıktı, hata, yan etki, kaynak kapsamı ve insan denetimi gereksinimleri bu kaydın parçasıdır. HTTP API, Rust SDK ve AI araç bağlantıları bu tanımdan üretilir. Bir sağlayıcıya özel araç açıklaması, asıl sözleşmeye yeni izin veya farklı davranış ekleyemez.

OpenAPI belgesi, JSON Schema şemaları, MCP araç tanımları ve Rust SDK üretimi planlanan çıktılardır. Henüz bu üreticilerin mevcut olduğu iddia edilmez. Üretilen yüzeylerin sürümü ve kaynak sözleşme özeti birlikte yayımlanır; elle düzenlenerek birbirinden uzaklaşmaları engellenir. Desteklenmeyen bir özellik, sessizce başka biçime dönüştürülmek yerine açıkça bildirilir.

Kimlik doğrulama ile yetkilendirme ayrı tutulur. Bir insanın adı, bir modelin sağlayıcısı veya “mimar” rol etiketi kendiliğinden izin vermez. Doğrulanmış özne, kaynak kapsamı, işlem, politika sürümü ve gerektiğinde onay kaydı birlikte değerlendirilir. İstek içindeki `authorization_ref`, güvenilir kayda çözümlenecek opak referanstır; izin beyanı olarak kabul edilmez.

## 2. Roller ve denetim biçimleri

Roller işlem bazında birleşebilir: gözlemci okuyabilir, planlayıcı değişiklik önerebilir, uygulayıcı izin verilen eylemi çalıştırabilir, onaylayıcı belirli planı onaylayabilir, yöneten kişi hedefleri ve yetki sınırlarını değiştirebilir. Aynı özne bazı işlemlerde birden fazla rol taşıyabilir. Çıkar çatışması bulunan işlemlerde öneren ile onaylayan ayrımı politika tarafından zorunlu kılınabilir.

**Human-in-the-loop, eylemden önce insan katılımıdır.** Politikanın gerektirdiği işlem, geçerli onay oluşmadan başlayamaz. Onay; tam plan kimliği ve revizyonuna, standartlaştırılmış yükün özetine, ilgili durum revizyonuna, politika revizyonuna, kaynak kapsamına ve sona erme zamanına bağlanır. Bunlardan biri değişirse eski onay taşınmaz. İnsan, okunabilir işlem farkını ve etkisini görür; aynı içerik AI’a yapılandırılmış olarak sunulur.

Onay tüketilirken bu bağlar yeniden doğrulanır. İlgili kaynağın koşullu güncelleme desteği varsa beklenen revizyon kullanılır. Harici bir sistemde değişmez durum varsayımı doğrulanamıyorsa uyumluluk garanti edilmiş sayılmaz; işlem yeniden planlanabilir veya belirsiz sonuç olarak ele alınabilir. “Onaylandı” ile “başarıyla uygulandı” ayrı durumlardır.

**Human-on-the-loop, yürütme sırasında gözlem ve müdahaledir.** İnsan görev ağacını, kullanılan model ve becerileri, olayları, süreleri ve kaynak tüketimini inceleyebilir. Durdurma komutu önce alındı olarak kaydedilir; yürütücünün gerçekten durduğu ayrıca bildirilir. Tarayıcının askıya alınması, ağ gecikmesi veya bitmiş GPU işi nedeniyle anında durdurma sözü verilmez. Kesme gecikmesi ölçülür; belirtilen bütçe sert gerçek zaman garantisi değildir.

**Human-in-command, hedef ve yetki yönetimidir.** İnsan amaçları, bütçeleri, devredilebilir işlem kapsamını, izin süresini ve yükseltme kurallarını belirler. Değişiklikler sürümlü politika üretir. Yetki geri çekme yeni adımları durdurur; başlamış işlemin sonucu, adapter’ın gerçek kesme yeteneğine göre raporlanır. Yerel model kendi yetkisini genişletemez. Bu üç biçim tek işlemde birlikte kullanılabilir: insan hedefi belirler, kritik eylemi önceden onaylar ve yürütmeyi izler.

## 3. Önerilen API yüzeyi

İşlem adları aşağıdaki alanlarda gruplanır. Ulaşım yolları ve yayınlanacak kesin adlar uygulama sırasında sözleşmeye sabitlenecektir.

| Alan | Tasarlanan işlemler ve temel sonuç |
|---|---|
| `inspect` | Sistem sürümü, görev durumu, kaynak revizyonu ve yürütme izini okuma |
| `capabilities` | Etkin izinleri, desteklenen işlemleri ve donanım yeteneklerini sorgulama |
| `tasks` | Hedef oluşturma, durum okuma, bütçe izleme, durdurma talebi |
| `plans` | Plan önerme, farkı inceleme, yeni revizyon oluşturma |
| `actions` | İzinli plan adımını yürütme ve kesin/belirsiz sonucu okuma |
| `events` | Sıralı olayları cursor ile takip etme ve kesinti sonrası devam etme |
| `approvals` | Onay isteme, belirli revizyonu onaylama, reddetme veya geri çekme |
| `models` | Model manifestini, kaynak özetlerini ve motor uyumunu inceleme |
| `datasets` | Veri sürümü, kökeni, kapsamı ve değerlendirme ayrımını yönetme |
| `training` | Eğitim planı oluşturma, izinli işi başlatma ve çıktısını izleme |
| `eval` | Sabit veri/sözleşmeyle değerlendirme işi ve raporu oluşturma |
| `releases` | Aday takım sürümü, sınırlı deneme, etkinleştirme ve geri dönüş |

Model, eğitim ve sürüm işlemleri bağımsız yetkilendirilir. Eğitim başlatabilen bir özne model sürümünü otomatik etkinleştiremez. Dil, algı, karar ve dünya modelleri; deterministik Rust becerileriyle birlikte takım sözleşmesine bağlanır. Dosya biçimini okuyabilmek, gözlem ve eylem anlamlarının uyumunu kanıtlamaz. Model değişiminde durum sıfırlama veya taşıma kuralı ayrıca değerlendirilir.

## 4. Her işlemin açıklayacağı bilgiler

İşlem kaydı; kimliğini, sürümünü, girdi/çıktı şemasını, okuduğu ve değiştirdiği kaynakları, yan etkilerini, gerekli yetkileri, desteklenen denetim biçimlerini ve önkoşullarını belirtir. Süre sınırı, eşzamanlılık sınırı, idempotency kapsamı, yeniden deneme davranışı, iptal noktaları ve telafi imkânı da aynı kayıtta yer alır.

Her açıklamada geçerli istek/yanıt örneği yanında reddedilecek örnekler bulunur: bilinmeyen alan, kapsam dışı kaynak, yanlış birim, süresi geçmiş onay, değişmiş plan özeti ve desteklenmeyen model işleci. İnsan açıklaması, ret nedenini ve düzeltme yolunu anlatır. AI yüzeyi aynı hatayı sabit kod, alan yolu ve izin verilen sonraki adımlarla verir. Açıklama metni şemanın yerine geçmez.

Bir capability kaydı bir izin taşıyıcısı değildir; o andaki etkin durumun görünümüdür. Yürütme anında politika yeniden değerlendirilir. Ekranda veya araç listesinde görünen bir işlem, o öznenin her kaynakta aynı işlemi yapabileceği anlamına gelmez.

## 5. Olaylar, hatalar ve tekrarlar

Yanıtlar `request_id`, ilgili görev/işlem kimliği, sözleşme sürümü ve durum revizyonu taşır. Uzun işler başlangıç yanıtında tamamlanmış gösterilmez. Olay akışı; kabul, başlangıç, ilerleme, onay bekleme, başarı, başarısızlık ve durdurma sonucunu ayırır. Olay kimliği tekilleştirme için kullanılır; bağlantı yenilendiğinde tekrar gelen olaylar yeni eylem başlatmaz.

Cursor opaktır ve ait olduğu akış kapsamıyla doğrulanır. Saklama süresi dolmuş cursor için açık hata ve yeniden durum okuma yolu sunulur. Sıralama garantisi önce görev veya akış içinde tanımlanır; sistem genelinde toplam sıralama vaat edilmez. Olay kaydının gerçekten tamamlanmış etkiyi mi, yalnız niyeti mi anlattığı belirtilir.

Hatalar önerilen `code`, `message`, `field_path`, `retryable` ve ilişkilendirme kimliği alanlarını içerir. Süre aşımı, yetki reddi, geçersiz onay, eski revizyon, bütçe aşımı, desteklenmeyen yetenek ve sonucu belirsiz işlem ayrı kodlardır. `retryable`, kör tekrar emri değildir; bekleme süresi, kalan bütçe ve idempotency koşulları birlikte kontrol edilir.

Yan etkili tekrarlar, özne/kapsam/işlem/yük özetiyle ilişkilendirilmiş idempotency anahtarı kullanır. Aynı anahtar farklı yükle gelirse reddedilir. İstemci zaman aşımı, sunucuda işlem yapılmadığını kanıtlamaz; önce sonuç sorgulanır. Harici etkiler için kesin bir kez çalıştırma garantisi verilmez. Süre ölçümleri kuyruk bekleme, çalışma ve toplam süreyi ayırır; son tarihin hangi saat otoritesiyle değerlendirildiği sözleşmede belirtilir.

**İptal geri alma değildir.** İptal, başlamamış adımı engelleyebilir veya çalışan işi desteklenen noktada durdurabilir. Yazılmış dosya, gönderilmiş istek veya etkinleştirilmiş sürüm için geri dönüş/telafi ayrı işlemdir. Sistem `cancel_requested` ile `cancelled` sonucunu ayırır; durdurulamayan veya sonucu bilinmeyen etkiyi saklamaz.

## 6. Örnek kullanım akışı

Önerilen senaryoda yerel eylemci aday model takımını sınırlı denemeye almak ister. Önce capability ve mevcut sürüm revizyonunu okur. Plan; hedef model takımının özetlerini, davranış sözleşmesini, donanım profilini, deneme kapsamını, bütçeyi ve geri dönüş sürümünü içerir.

Politika onay istiyorsa insan değişiklik farkını inceler; onay tam plan ve yük özetine bağlanır. Uygulayıcı yürütme talebini idempotency anahtarıyla gönderir. Sistem yetkiyi, revizyonları, kanıtları ve onayı yeniden doğrular. Ardından eylem başlatılır; insan ve AI aynı olay akışından ilerlemeyi izler. Eşik aşılırsa politika yeni adımları durdurabilir ve ayrıca yetkilendirilmiş geri dönüş işlemini başlatabilir. Bu anlatım çalıştırılmış örnek değildir.

## 7. Dokümantasyon ve uyumluluk kabulü

İnsan belgeleri görev odaklı öğreticiler, rol açıklamaları, onay ekranı örnekleri ve hata çözüm yolları içerir. AI belgeleri kesin şemaları, yetenek listesini, birimleri, kısıtları, geçerli/geçersiz yükleri ve sürüm geçişlerini içerir. Aynı işlemin anlamı iki belgede değişemez.

Temel tasarım kayıtları: [ihtiyaç şeması](../../ai-team-protocol/request.schema.json), [ihtiyaç örneği](../../ai-team-protocol/request.example.json) ve [model takımı sürüm şeması](../../ai-team-protocol/model-set-release.schema.json). Bunların mevcut olması çalışan API anlamına gelmez; şema doğrulaması yetki ve davranış doğrulamasının yerini tutmaz.

Hızlı başlangıç ve “test edilmiş örnek” etiketi ancak uygulama tamamlandığında yayımlanır. Her örnek çalıştırıldığı kod sürümüne, ortamına ve sonucuna bağlanır. Uyumluluk kapısı; üretilen belgelerin eşleşmesini, örnek yük doğrulamasını, ret senaryolarını, tekrar davranışını, olay devamını ve gerçek iptal sonuçlarını denetler. Çalıştırılmayan donanım veya adapter için başarı iddiası yapılmaz. Bu belge tasarım sınırını tanımlar; herhangi bir erişim bilgisi, çalıştırma yetkisi veya dış işlem oluşturmaz.
