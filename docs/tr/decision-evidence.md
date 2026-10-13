# Karar kanıtı ve ölçülen öz araç döngüleri

G50, değişmez bildirilen seçenekler ve mühürlü gerçek araç anlık kayıtları ekler. [Türlü araçlar](typed-tools.md), gerçek G10/G11/G13 yetki, durdurma ve kurtarmayı; [proje verisi](project-data.md) beş ayrı amacı korur. Bu native öz okuma/ekleme sürücüsüdür; model, genel agent döngüsü, graph motoru veya nedensel açıklama değildir.

## Bildirim ve veri soyu

Bildirim aynı eylem ve skor birimlerinde1–8 benzersiz yol, seçilmiş eylem/araç profili/revizyonu, kısıt beyanları, isteğe bağlı tam bildirilen model profili/artifact/revizyonu ve kalibre edilmemiş ya da bildirilen kalibrasyon referansı içerir. Fayda tamsayıları±1.000.000.000, bildirilen olasılık baz puanları0–10.000 ile sınırlıdır. Tamsayı birimleri NaN/sonsuzluk kodlayamaz. Kısıt beyanı reddedilen yol seçilemez. Güven, kalibrasyon referansı ve model artifact'ı doğrulanmamış bildirim olarak kalır; burada model çalışmaz ve atıf nedensellik kanıtlamaz.

Yalnız özgün mühürlü kabul oturumu bildirimi ilk gerçek adımdan önce bağlayabilir. Eylem/profil/revizyon eşleşmesi zorunludur; değişen bağ ve geç bağlama reddedilir. Bağlamadan ve her kanıt okumasından önce gerçek kapsamlı rapor/kaynak yetkisi kontrol edilir. Salt okuyucu gözlemci seçim yazamaz. Önceden bağlı aynı bildirim ve yürütme replay'i özgün örnekleri korur. Araç raporları gerçek görev/yürütme/eylem/kanıt ilişkisini ve bekleyen/tamamlanan/başarısız/durdurulan/kısmi/bilinmeyen ayrımını sağlar; çağıranın sonuç nesnesi kabul edilmez. Sabit uygulama kimliği `owned:typed-tool-driver:v1`, yürütülen deterministik sürücüyü sabitler; çalışmamış G46 model-döngü denetleyicisini değil.

Tutulan tüm girdiler G54'ün güncel kaynak/oturum/ata kontrolleriyle çıkışı sınırlar. Karar anlık kaydı geçici yerel Execute kanıtıdır. Otomatik toplama, eğitim, dış açıklama veya ekosistem ihracı yapılmaz; bu izinler ayrıdır. Uygun sır olmayan kimlikler ve bildirilen metaveri, güvenilir alım ve özel journal önkoşuldur. Dönen kayıt geri çağrılamaz; bunların toplamı yerel hesaplamadır ve saklama/paylaşım izni yaratmaz. Yalnız yeni host türlü soy kuralı sürer: kurtarma eksik soyu literal diye yeniden etiketleyemez ve bilinmeyen etkileri sürdüremez.

## Ölçümler ve sınırlar

Standart kitaplığın gerçek monoton gözlemleri kabulü, en fazla9 eylem/durdurma adımını, istemci aralıkları dahil kabulden terminale süreyi ve kabul edilen iptal isteğinden terminal duruşa süreyi kapsar. Başarılı idempotent replay yeni örnek eklemez. Kaynak reddedilen çağrı dışlanır; başarısız/kısmi/iptal sonuçlar tamamlanan döngü dağılımına giremez. En fazla128 yürütme tahliyesiz tutulur.

Facade'in ilk yeni kabulü ilk/soğuk host çağrısıdır; sonrakiler sıcak/tekrar host çağrılarıdır. Süreç başlangıcı, yüklenmemiş ağırlık, cache boşaltma veya model ısınması ölçülmez. Dağılım, aynı canlı facade'den gerçek handler/profil/revizyon/payload/eylem bayt bütçesi aynı2–128 farklı tamamlanan kayıt kabul eder; ilk çağrı ve en az bir tekrar gerekir. Farklı host, girdi, tekrar kayıt, bitmemiş veya durdurma sonuçları reddedilir. Sıcak ham örnekler korunur; en yakın sıra p50/p95/p99 sıralı tekrarlardan hesaplanır. Başarılı döngü/saniye, tamamlanan sayıyı çağrılar arası aralık dahil gerçek en erken kabul–en son terminal duvar süresine böler. Seri gözlenen hızdır; eşzamanlı kapasite veya SLA değildir. Görev kimlikleri, büyüyen depolama, ortam yükü ve yetki/istemci işi süreyi etkileyebilir; anahtar donanım eşdeğerliği veya başka uygulamayla kontrollü kıyas kanıtlamaz.

Bildirilen görev payload bayt tavanı ayrı sunulur. Kaynak rezervasyonu, fiziksel süreç RSS'i ve fiziksel VRAM açıkça bulunamaz (`None`); sıfır değildir ve payload/wire bütçesinden çıkarılmaz. G21 kaynak hesabı ve G18/G22 donanım kayıtları ayrı kanıttır; sayaç işi model belleği veya GPU maliyetini kalibre edemez.

## Doğrulama

Taşınabilir olumsuz testler boş/tekrar/8 üstü/eksik/reddedilen seçimleri, skor sınırını ve doğrulanmamış kalibrasyon/model/nedensellik durumunu kapsar. Native testler gerçek kapsamlı seçim–okuma/ekleme gözlemini, revizyon uyuşmazlığını, salt okuyucu yazma reddini, yeniden/geç bağlamayı, özgün replay sürelerini, kaynak çekmeyi ve reddedilen okuyucuyu kapsar. Gerçek64 seri sayaç döngüsü64 sonucu,63 tekrar örneğini, açık en yakın sıraları ve pozitif ölçülen hızı doğrular. Bir doğrulanmış etkiden sonra gerçek kapsamlı durdurma, kısmi sonuç ve duruş gecikmesini kaydeder; tamamlanan dağılımdan dışlanır. Bilinen sayaçlar etkiyi doğrular; süre koşullarında kırılgan mutlak hız eşiği yoktur.

Yerel hedef ARM64 macOS/Rust1.99.0'dır. Bellek journal'ı ve mevcut benzetilmiş host-insan yetkisi kullanılır; yeni fiziksel IO, gerçek kişi girişi, GPU/model/eğitim/provider/G40 denemesi ya da motor/bağımlılık sağlanmaz. Kabulden önce güncel tam kapılar, tam commit CI ve gerçek ayrı inceleme zorunludur. Arayüz bağlantısı, kalıcı karar veri seti, gerçek model-döngü yürütmesi ve performans kıyasları sonraki kapılarda kalır.
