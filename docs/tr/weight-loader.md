# G28 birinci taraf ağırlık baytlarının yüklenmesi

[İngilizce](../en/weight-loader.md). Güvenli kendi Rust `eylema-model` crate'imiz sınırlı beyan edilmiş Safetensors alt kümesini değişmez sahiplenilmiş baytlara yükler. G28 native testleri bayt/layout/servis kanıtıdır; model çalıştırma değildir. [Tasarım](g28-weight-loader-design.md) ve [plan](https://github.com/eylema-dev/devguide/blob/codex/g28-weight-loader-plan/docs/plans/g28-weight-loader.md) kabul edilen kapsamı bağlar; teslim için PR/güncel CI/ayrı inceleme/main kanıtı hâlâ gerekir.

## Çağıran sözleşmesi

`TensorSpec::new` düz ASCII adları ve pozitif rank1..4 şekilleri doğrular. `WeightLayout::new` tam kanonik G27 manifest digest'ini, yapılandırılmış ağırlık bileşenini ve sıralı benzersiz beklenen tensor haritasını bağlar. Yalnız tek tip saklanan BF16/F32, manifest `safetensors.v1`, `bfloat16`/`float32`, `none` quantization kabul edilir. Beklenen haritanın yayıncı/mimari kökeni çağıran girdisidir; bağımsız model bütünlüğü veya hak kanıtı değildir.

`load` çağıranın sahip olduğu izinli Read uygulamasını ve açık LoadBudget alır. Girdi tüketmeden tam beyan edilen bileşen boyutunu, artifact/resident/okuma sınırlarını ve değişmez manifest/layout eşitliğini denetler. Bütün artifact rezervasyonundan önce sınırlı header'ı okuyup doğrular; tam prefix/header/payload dosya sırasında hash'lenir, son EOF ve beklenen SHA256 gerekir, sonra LoadedWeights döner. Hata kısmi snapshot döndürmez. Mevcut assignment/schema/task/journal sözleşmeleri değişmez; model çalıştırılabilir hâle gelmez.

Accessor'lar değişmez ödünç tensor dilimi/spec ve gerçek artifact/manifest/layout digest'leri sağlar. Dönen depolama yaşam süresini sahiplenir: girdi dosyasını sonradan değiştirmek/silmek bunu değiştiremez. Kütüphane veya hash eşleşmesi dosya/proje/execute/collect/train/disclosure/export yetkisi vermez. Yol/ağ açma, harici modele düşme, tokenizer, dönüşüm veya çıkarım yoktur.

## Açık sınırlar ve desteklenmeyen durumlar

Bütün artifact<=2147483648bayt, header<=1048576,1..1024tensor,rank1..4,pozitif checked boyut. Adlar<=128ASCIIbayt, decode edilmiş key eşitliği/benzersizliği; metadata<=128string çift,key<=128decode bayt,value<=4096; derinlik<=6(root0),16384value node. Bilinmeyen/duplicate/eksik tensor alanı, bozuk UTF8/escape/surrogate/sayı, uyumsuz dtype/shape, aralık dışı/çakışan/boşluklu/indekslenmemiş payload reddedilir. İsteğe bağlı metadata doğrulanıp atılır; talimat veya sıradan debug çıktısı olmaz.

Son EOF dahil okuyucu çağrısı<=65568; kesinti<=16; body parçaları<=65536bayt. Artifact ayrılmadan önce çağıranın resident içerik sınırı tam artifact boyutu ve sınırlı header/parser/layout/scratch için4194304bayt payı kapsar. Sahiplenilmiş büyüyebilir içerik belleği hata döndürebilen rezervasyon kullanır; OOM/allocator/OS muhasebesi ve çağıran/diğer işler gözlenmiş fiziksel RAM değildir. Büyük boyut reddi fixture'ları büyük buffer ayırmaz.2GiB tavan mantıksal hedeftir; ölçülmüş2GiB kapasite veya sığma önerisi değildir.

Scalar/boş/packed/quantized/sharded biçimler reddedilir. BF16/F32 baytları opak kalır: dosya biçimi sonlu olmayan değerler içerebilir; dönüşüm/sonluluk/operatör uyumu sağlanmaz. Sonraki model/profil kapıları tam tensor mimarisini, tokenizer'ı ve sayısal desteği kanıtlamalıdır. Genel senkron Read zamana bağlı iptal garantisi veremez; çağıran sınırlı bloklama/deadline reader sağlamalıdır. Snapshot süreç içidir; kalıcı depolama ve garantili silme sağlanmaz.

## Native servis sınırı

`eylema_platform::sha256_stream::Sha256Stream` özel macOS CommonCrypto context'ini sahiplenir; beyan edilen toplam1..2147483648bayt, parça<=1048576. Geçersiz/aşırı update zehirler; boş update dahil sonraki update/finish başarısız olur. Finish durumu tüketir. Klonlama, kriptografi algoritması veya gizli motor yoktur. Mevcut tek seferlik SHA4MiB wire sınırını korur. Desteklenmeyen native/Wasm servisUnavailable döndürür; mevcut seçili portable library derlemesi bu yükleyiciyi çalıştırma değildir.

Kurulu SDK ABI'si geçici C syntax/layout assertion ve Rust layout kontrolleriyle bağımsız doğrulandı: u32CC_LONG, context104bayt/align4,count/hash/wbuf offset0/8/40. Senkron Init/Update/Final status1 ve canlı checked girdi/çıktı/context yaşam süresi ister. [Apple API](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/CC_SHA256.3cc.html) ve SDK bağımlılıklar olarak kalır. Kontrollü başarısız-status unit testleri güvenli işlemeyi kanıtlar; gerçek OS servisi zorla başarısız yapılmadı.

## Gerçek sınırlı kanıt

Yedi yeni stream testi (beş integration/iki unit) empty/abc/million-a bağımsız vektörlerini, parça sınırlarını, dondurulmuş Python hashlib ile5MiB tekrar eden baytları, eski wire reddini, toplam/aşırı sınırları, poison/status ve checked layout'u çalıştırır. On üç native model testi bağımsız dondurulmuş F32/BF16 artifact dilimleri ile kanonik manifest/layout digest'lerini, bozuk sınırlı header'ı, tam dosya karşılaştırmasını, kısmi/kesintili/hatalı/fazla girdiyi, okumadan önce bütçeleri ve gerçek küçük sentetik dosyanın değiştirilmesi/silinmesini çalıştırır. Dört geçici derlenmiş kaynak mutasyonu atlanan digest, atlanan prefix hash, eşit baytlı yanlış shape ve kabul edilen overlap'i sınar; davranışsal başarısızlık ve geri yüklenmiş green kanıtı özel tutulur.

Yalnız kendi127/134bayt sentetik artifact'lerimiz ve sınırlı sentetik hash verisi kullanıldı; gerçek ağırlık/model/provider/eğitim çağrısı yoktur. Testler gerçek Qwen yüklemesi, mimari doğruluğu,2GiB bellek/throughput,GPU/model/eğitim performansı veya tam Wasm/başka cihaz çalışmasını kanıtlamaz. Kaynak/main CI ve son gerçekten ayrı inceleme zorunlu ayrı kanıtlardır; bu notlardan çıkarılmaz. G40 hatası/bütçeleri korunur.

Ayrı inceleme,1..7bayt olarak bildirilen artifact’ların redden önce8bayt önek okuyabildiğini buldu. Okumada panic veren BF16/F32 regresyonu önce başarısız oldu; yükleyici artık bu boyutları okumadan reddeder. Teslim için düzeltilen sürümün kontrolleri ve ayrı incelemesi gerekir.
