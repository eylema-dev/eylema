# Yerel CLI

Native `eylema` komutu artık G14 loopback servisinin G15 giriş noktasıdır. Varsayılan oturumu macOS üzerinde olağan yerel aktördür. G73 aşağıdaki açık OS cihaz sahibi yolunu ekler; bu uzaktan kimlik değildir.

## Başlatma ve kullanım

G14 servisini IPv4 loopback ve geçici bir portta başlatmak, kalıcı günlüğü `~/Library/Application Support/Eylema` altında oluşturmak veya yeniden açmak için bir terminalde `eylema start` çalıştırın. Komut ön planda kalır. `EYLEMA_DATA_DIR`, test veya açıkça yönetilen profil için başka bir yerel veri dizini seçer. Oturum dosyası, grup/diğer kullanıcı izinleri kapalı dizinde `0600` kipindedir; ekrana basılmaz, argv içine konmaz, loglanmaz ve günlüğe yazılmaz. `eylema stop service`, ön plandaki süreçten durmasını ve worker’ını birleştirmesini ister. Sürecin beklenmedik sonlanması düzgün kapanış kanıtı değildir.

```sh
eylema start
# Başka bir terminalde:
eylema doctor
eylema run demo 7
eylema inspect task <task-id>
eylema inspect execution <execution-id>
eylema stop service
```

`run demo [delta]`, yalnızca Eylema’nın sahip olduğu görev durumundaki değeri artıran sınırlı bir görev oluşturur. Delta 1–1.000.000 arasındadır. Dosya sistemi, shell, ağ veya başka dış etkisi yoktur. `create <request-file>` ve `run <request-file>`, var olan sürümlü EYL taşıma codec’iyle kodlanmış, boyutu sınırlı ikili istekleri alır. `inspect`, kararlı JSON kayıtları yazdırır ve servisin görev/yürütme kimliklerini ve makine hata kodlarını kullanır. Hatalar `{"status":"error","code":"..."}` biçiminde yazdırılır ve sıfırdan farklı çıkış kodu döndürür.

## Yetki ve sınırlar

`start`, `state:cli` kaynağıyla sınırlı, insan olmayan bir CLI oturumuna `create_task`, `read`, `execute_action` ve `observe_events` izinlerini verir. Yerleşik demo için yeterlidir; bu kaynak kapsamı dışındaki istekler servis tarafından reddedilir. `stop <execution-id>`, web servisiyle aynı tipli iptal isteğini gönderir. İptal ancak güvenilir host tarafından bağımsız olarak insan doğrulaması yapılmış ve iptal izni verilmiş geçerli oturumda başarılı olur. Standart CLI oturumu bu iddiayı taşımaz ve `FORBIDDEN` alır; CLI bir bayrak veya istemi insan doğrulamasına yükseltmez. Ayrı host tarafından verilmiş insan doğrulama fixture’ı kullanan test, yetkili iptalin etkili olduğunu doğrular.

`doctor`, var olan özel veri dizinini, oturumu ve kimliği doğrulanmış loopback servisini; veri oluşturmadan veya onarmadan kontrol eder. `start`, güvenli olmayan dizin, sembolik günlük bağlantısı, desteklenmeyen platform, kilit çakışması veya geçersiz günlükte fail-closed davranır. Geçmişi kesmez, taşımaz veya sessizce kurtarmaz. Yeni günlük özeldir ve servis açıkken kilitli kalır.

## Kanıt sınırı

G15’in native macOS alt-süreç testi gerçek CLI servisini başlatır, tipli görev oluşturup çalıştırır, ortak kimlik ve makine hatalarını denetler, ayrı host tarafından verilmiş insan oturumuyla yetkili iptal ister ve sonucunu doğrular, standart insan olmayan oturumun iptalini reddeder, servisi düzgün durdurup yeniden başlatır ve günlükteki durumu inceler. Bu testler GUI, gerçek kullanıcı kimlik sağlayıcısı, uzak TLS, yayın, GPU/model/eğitim veya fiziksel güç kaybında kurtarma kanıtı değildir.

G73, açık `start --human` ve `human-proof <new-private-directory>` ekler: [cihaz sahibi kapsamı, süre ve kanıtı](real-human.md). Mevcut düz CLI olağan yerel aktör kalır. Fiziksel doğrulama yerel CI’dan çıkarılmaz.
