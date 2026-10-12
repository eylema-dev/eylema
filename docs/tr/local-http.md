# Yerel HTTP servisi — G14

[English](../en/local-http.md). Görev revizyonu2; kabul edilmiş G09–G13 sonrasında [Goal issue16](https://github.com/eylema-dev/eylema/issues/16).

Birinci taraf native `eylema-web` servisi, kalıcı görev işlemlerini IPv4 `127.0.0.1` üzerinde sunar. Gerçek macOS soket/dosya testleri görev oluşturup çalıştırır, özgün yürütmeyi ve olayları inceler, etkili iptal ister ve yetkisiz veya bozuk istekleri reddeder. Genel CLI ve tarayıcı konsolu G15/G16 kapsamındadır; buradan tarayıcı, gerçek kişi oturumu, uzak TLS, GPU veya model kabulü çıkarılmaz.

## Bileşim ve yetki

Native sunucu, uygulamaya alan host’un sağladığı opak `DurableHost` yüzeyini sahiplenir. Kapsamlı host işlemlerini çağırır; değiştirilebilir runtime veya günlük tutamacı almaz. Doğrudan birinci taraf bağımlılıkları contracts, host, journal depolama/hash trait’leri, runtime servis/grant türleri ve platform entropisidir. Bu, önerilen bileşen tablosunu gerçek native bileşime uyarlar; runtime ve journal üretim bağımlılıkları contracts yönünde kalır. Dış Cargo paketi veya build script yoktur. Taşınabilir istek codec’i `eylema-contracts::transport` içinde yaşar; native soket servisini Wasm’a almak, seçili taşınabilir kütüphane kapısının gereği veya desteklenen davranışı değildir.

Yalnız güvenilen host kodu kapsamlı oturum kaydedebilir veya bağımsız doğruladığı insanı tasdik edebilir. HTTP üzerinden kayıt, grant, host anahtarı, günlük yolu, sağlayıcı veya insan statüsü işlemi yoktur. Kayıt, standart macOS entropisiyle ayrı bearer ve CSRF kimlik bilgileri döndürür. Kimlik bilgileri süreç belleğinde kalır, debug gösterimi maskelenir ve bu adaptör tarafından günlüğe, URL’ye veya log’a yazılmaz. Teslimlerini host korumalıdır; bu yerel kimlik bilgileri kişi kimliğini kanıtlamaz. Testler açıkça simüle edilmiş doğrulanmış insan host fixture’ı kullanır.

Her istek tam bağlanılan `Host`, tam `Origin`, `Authorization: Bearer` kimlik bilgisi ve eşleşen `X-Eylema-CSRF` gerektirir. Sağlanırsa `Sec-Fetch-Site`, `same-origin` olmalıdır. Oturum süresi gövde alımından önce ve sonra denetlenir. İzinler ve tam görev kapsamları adaptör ve host/runtime tarafından denetlenir. Kabul, eylem principal’larını kayıtlı aktöre de bağlar. Okuma/olay/kanıt istekleri kapsamlı kalır; iptal gerçek opak insan oturumunu ve iznini gerektirir. Kabul edilmiş politika, denetim, onay, geri alma, bütçe ve idempotency kalıcı host üzerinden sürer. Sıradan görev verisi yetki oluşturamaz.

## Kapalı aktarım profili

Uygulanan tek yol, bir ondalık `Content-Length` ve `Content-Type: application/eylema` ile `POST /v1/request HTTP/1.1` yoludur. Her bağlantı bir istek taşır ve yanıttan sonra kapatılır. Bu bilinçli olarak sınırlı bir yerel adaptördür; önerilen API kataloğundaki tüm yolların veya genel HTTP/proxy birlikte çalışabilirliğinin uygulaması değildir. Bilinmeyen işlem etiketleri ve yük alanları, büyük/küçük harften bağımsız yinelenen başlıklar, geçersiz/kontrol/ASCII dışı başlık değerleri, sorgulu/mutlak biçimli yollar, parçalı aktarım, içerik kodlaması ve `Expect` reddedilir. Kökenler arası CORS/ön istek, TLS, WebSocket, akış veya keep-alive yüzeyi yoktur. Bildirilen gövdenin ötesindeki tamponlanmış baytlar reddedilir; sonradan gelen baytlar kapatılan bağlantıda ikinci işlem olamaz.

Birinci taraf EYL codec’i istekleri `[1, operation_tag, payload]` olarak kodlar. Tipli görev/öneri kayıtları mevcut alan codec’iyle iç içe baytlardır; aktarım adaptörü sürümlerini veya yetki anlamlarını değiştirmez. Onay kimlikleri64, olay sayfaları1–256 ile sınırlıdır.

| Etiket / istek | Yetkili işlem ve başarı değeri |
|---|---|
| 0 / `Create` | Host’un yapılandırdığı değişmez denetimle görev oluştur; `Null` |
| 1 / `Admit` | Özgün tipli görev/öneri ve onay kimliklerini kabul et; yürütme kimliği |
| 2 / `Task` | Tipli güncel görev baytları, değer, yapılan adımlar, ücretlenen adımlar ve olaylar |
| 3 / `Execution` | Tipli özgün görev/öneri/yürütme kaydı; kimlik ve özgün kabul bağlamı korunur |
| 4 / `Events` | İsteğe bağlı olay kimliğinden sonraki kapsamlı sıralı tipli olaylar, sınırlı sayfa |
| 5 / `Observation` | Kanıt/olay/görev/yürütme referansları, zaman ve gözlenen önce/sonra değerleri |
| 6 / `Cancel` | Doğrulanmış principal/zaman içeren istek alındısı veya açıkça zaten terminal durum |

Yanıtlar EYL `['ok', value]` veya `['error', stable_code]` biçimindedir. Başarı200 kullanır; kabul başarısı yürütme tamamlanması değildir. Aktarım durumları400 bozuk,401 doğrulanmamış/süresi dolmuş,403 yasak,405 desteklenmeyen,408 alım süresi,413 boyut sınırı,409 bilinen runtime reddi ve503 kullanılamayan/yapılandırma/depolama hatasıdır. Yanıt başlıkları `Connection: close`, `Cache-Control: no-store`, `X-Content-Type-Options: nosniff` ve kısıtlayıcı `Content-Security-Policy` içerir. Hatalar istek verisini veya kimlik bilgilerini geri yansıtmaz.

## Sonlu sınırlar ve hata davranışı

| Kaynak | Varsayılan | Kabul edilen yapılandırma aralığı |
|---|---|---|
| Başlık baytları | 8192 | 128–16384 |
| Başlık sayısı / adı / değeri | 32 / 96 bayt / 2048 bayt | Sabit |
| İstek gövdesi | 64KiB | 32 bayt–64KiB |
| Yanıt gövdesi | 4MiB | 256 bayt–4MiB |
| Mutlak alım / yanıt yazma süresi | Her biri5 saniye | Her biri pozitif, en fazla10 saniye |
| Oturum süresi / tutulan kayıtlar | 1 saat / 16 | Pozitif, en fazla24 saat / 1–16 |
| Native yoklama aralığı | 10ms | 1–100ms |
| Reddedilen alım kapatma payı | 5ms içinde en fazla8KiB atılır | Sabit |

Mutlak alım süresi başlıkla gövdeyi birlikte kapsar ve her baytta sıfırlanmaz. Kabul edilmiş bağlantılar açıkça bloklayan IO ve kalan süreye bağlı soket sınırları kullanır; dinleyici bloklamaz. Kesilen IO tekrarları sonludur. Boyut/doğrulama/biçim kontrolleri dispatch’ten öncedir. Bilinen anlamsal runtime hatalarının G13 altında kalıcı hata kaydı olabilir; ret, her hatanın sıfır günlük baytı yazacağı sözü değildir. Belirsiz yanıt teslimi kabul edilmiş kalıcı işlemi geri almaz: yeni görev/yürütme uydurmak yerine bilinen kimliği sorgulayın veya özgün idempotent kabulü yineleyin.

Native döngü en fazla bir bağlantıyı, ardından önceden yetkilendirilmiş bir kuyruk adımını işler. İstemci kapandığında sürer; boş yoklama iş-yok kaydı eklemez. Runtime eylemleri özgün kapsamlı yetkilerini yeniden denetler. Yavaş bağlantı sonraki adımı alım sınırı içinde geciktirebilir. IO sınırları kesin kernel, senkronizasyon, eylem veya iptal gecikmesi kanıtlamaz. Host/depolama hataları açıktır ve uygulamaya alan döngüyü durdurabilir; süreç kapatmayı sahibi yönetir. Geçmiş, oturumlar ve günlük kapasitesi G13 altında sonlu kalır; otomatik döndürme, kimlik bilgisi yenileme veya geçmiş silme yoktur.

## Çalıştırılan kanıt ve yeniden üretim

Ortam: seçilmiş M4 Pro/24GB Mac’te macOS27.0.1 ARM64, Rust1.99.0 ve Python3.14.8. Değişmeyen Rust kapısı yedi birinci taraf paketi, fmt, uyarıları reddeden clippy,105 native test, native derleme ve seçilmiş contracts/platform Wasm derlemesini geçti. Enjekte edilmiş deterministik saat/okuyucu testi, başlık ilerlemesinin gövde okumalarında kullanılan aynı mutlak süreyi tükettiğini doğrular; özel süreyi her okumada sıfırlayan mutant bu regresyonda başarısız olur. Native takım ayrıca eksik başlık/gövde isteklerinin günlük değişikliği olmadan gerçek408 yanıtı aldığını doğrular. Sunucu zaman aşımı iddiası, istemci bekleme/planlama süresini ölçmez. Yeni testler iki aktarım durumunu, özgün kapsamlı yürütme okumalarını, yabancı host için bekleyen iş kontrollerini ve bağımsız sınanan çok sayıda senaryo içeren bir native HTTP kabul takımını kapsar.

```sh
env -u RUST_LOG .venv/bin/python tools/check_rust.py
cargo run --package eylema-web --example http-proof --locked --offline
```

Native kanıt gerçekten sahipli geçici loopback portlarına bağlanır ve standart OS entropisi/SHA, dosya/üst dizin senkronizasyonu ve macOS tam senkronizasyonuyla sahip kontrollü geçici0700 dizinler/0600 kilitli günlükler oluşturur. İki sahipli adımdan sonra sonuç değeri5, aynı kabulde korunan yürütme kimliği, tipli olay/kanıt, her istemci bağlantısı koptuktan sonra native devam, boşta yazmama, kimlik bilgilerinin günlük/debug dışında kalması, reddedilen host/origin/bearer/CSRF/izin/kaynak/insan iddiaları, bozuk/çerçeveleme/aşırı büyük istekler, gerçek eksik başlık/gövde reddi ve deterministik ortak başlık/gövde mutlak süresi ile sıfırlama mutantının reddi, alım sırasında oturum bitimi, etkili iptal, desteklenmeyen bind ve sınırlı kayıt doğrulanır. Kanıtın sahip olduğu dinleyici/dosyalar temizlenir; ilgisiz checkout ve dinleyiciler korunur.

Python/statik kontroller, seçilmiş Wasm derlemesi ve native soket davranışı ayrı kanıtlardır. Gerçek tarayıcı akışı ve varsayılan Application Support CLI bileşimi G15/G16 kapsamındadır; gerçek kişi doğrulaması, uzak erişim/TLS, fiziksel güç kaybı, dış etkiler, GPU/model/eğitim, performans, Linux ve ikinci fiziksel cihaz `not_run` durumundadır. PR/metin olayı CI’ı, yeni ayrı tam-baş incelemesi ve merge/main kanıtları gerçekten kaydedilene kadar teslim kapılarıdır.

İncelenen platform/protokol anlamları için referanslar: [RFC9112](https://www.rfc-editor.org/rfc/rfc9112.html), [Rust TcpStream](https://doc.rust-lang.org/std/net/struct.TcpStream.html), [Fetch Standard](https://fetch.spec.whatwg.org/). Kod yukarıdaki sınırlı profili uygular; bu belirtimlerin tümüne uygunluk iddiası değildir.
