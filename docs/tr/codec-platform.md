# G09 — Kanonik codec ve platform hizmetleri

[English](../en/codec-platform.md). Çalışma zamanı sözleşmesi **0.1.0**, EYL codec **1**, yayımlanmamış Rust paketleri **0.1.0**. [Görev planı](../../tools/work-plans/g09.json), [tipli sözleşmeler](typed-contracts.md), ana issue [eylema#11](https://github.com/eyleme-dev/eyleme/issues/11).

## Wire biçimi ve sınırlı ayrıştırma

Birinci taraf, güvenli Rust [codec](../../crates/eylema-contracts/src/codec.rs) tam yedi byte `45 59 4c 01 00 01 00` ile başlar: ASCII EYL, codec sürümü 1, çalışma zamanı sürümü 0.1.0. Sürümler tam eşleşmelidir. Değer etiketleri bir byte; uzunluk/adet unsigned big-endian u32; signed/unsigned sayılar ayrı, sabit sekiz byte big-endian değerlerdir.

| Etiket | Değer |
|---|---|
| 0 / 1 / 2 | Null / false / true |
| 3 / 4 | Signed i64 / unsigned u64 |
| 5 / 6 | Uzunluk önekli opak byte dizisi / UTF-8 metin |
| 7 | Adet önekli sıralı liste |
| 8 | Adet önekli map: uzunluk önekli UTF-8 anahtar, ardından değer |

Map anahtarları boş olamaz, kontrol karakteri içeremez, en fazla 96 byte olmalıdır; benzersizdir ve UTF-8 byte sırasına göre dizilir. Encoder anahtarları sıralar; decoder sırasız/tekrarlı anahtarları reddeder. Listelerin sırası korunur. Metin byte’ları aynen kalır: birleşik ve ayrıştırılmış Unicode dizeleri farklıdır; örtük normalizasyon veya float dönüşümü yoktur. Float için desteklenen etiket yoktur.

Sınırlar: toplam wire girdi/çıktı 4 MiB, her byte değeri 1 MiB, her metin 4096 byte, derinlik 32 (kök derinliği 0), kök dahil toplam 65536 değer düğümü. Container da bir düğüm sayılır. Adetler, uzunluk aritmetiği, kalan minimum wire byte’ı ve düğüm bütçesi, decoder bellek ayırmadan önce kontrol edilir. Bilinmeyen etiket/sürüm, geçersiz UTF-8, artan/kesilmiş girdi, aşırı derinlik/düğüm/toplam byte ve geçersiz anahtar başarısız olur. Ek wire sınırları, tekil G08 kayıtları kendi sınırlarına uysa bile birleşimini reddedebilir; bunlar protokol sınırıdır, ölçülmüş kapasite değildir. Çağıranın Rust Value oluşturması tek başına güvenilmeyen girdinin kabul yolu değildir.

Sabit referans map `{"b": null, "a": unsigned(42)}` şöyle kodlanır:

```text
45594c010001000800000002000000016104000000000000002a000000016200
```

## Tipli kayıtlar ve onay payload’ı

[Tipli adaptörler](../../crates/eylema-contracts/src/codec/typed.rs) tüm G08 task/action/proposal/execution/effect/event/error, sürüm, ID, kaynak, yetki, gözetim, bütçe ve hedef alanlarını kapsar. WireRecord türleri task, proposal, execution, event ve error’dur. Proposal kaydı task anlık görüntüsünü; execution kaydı ayrıca proposal’ı taşır. Böylece decoder mevcut constructor’ları ve iç kaynak/revizyon/politika/hedef/bütçe bağlarını yeniden denetler. Bilinen tüm alanlar zorunludur (opsiyonel değerler açık null kullanır); bilinmeyen veya eksik alan, desteklenmeyen enum/sürüm ve daraltmada integer taşması başarısız olur. Kaynak, izin, checkpoint, capability ve evidence kümeleri kanonik sıralanır; decoder sırasız kümeleri reddeder. Action/effect listelerinin sırası anlamlı kalır. Sırasız kaynak Vec farklı sırada olsa da kanonik round-trip byte’ları sabittir.

`canonical_approval_payload(task, proposal)` proposal’ın verilen task’a bağını yeniden doğrular, `approval_payload` türünü kullanır ve tüm tipli task/proposal/action etki beyanını içerir. Yalnız taşıma RequestContext alanları (request/correlation ID’leri ve tekrar eden context sürümü) çıkarılır. Wire başlığı çalışma zamanı 0.1.0 bağını; operation sürümleri kendi bağını korur. Task/proposal/action kimlikleri, amaç, revizyonlar, operation, opak payload, kapsam, yetki/politika/gözetim/capability, bütçe, hedef, idempotency ve action sırası bağlı kalır. Opak payload byte’ları ayrıştırılmaz veya yeniden yazılmaz. Taşıma ID değişikliği digest’i korur; etki payload’ı veya action sırası değişikliği değiştirir. Hash üretimi codec’ten ayrıdır. Eşleşen hash, doğrulanmış yetki veya kabul edilmiş onay değildir; gerçek politika/onay davranışı G11’e aittir.

[Native tüketicideki](../../crates/eylema-platform/examples/codec-proof.rs) geliştirme fixture’larının tam SHA-256 referansları: task `e2806de4884f8cf91c7e859ed741607545e2062674658d3b45fb69a4480d5f4d`, onay payload’ı `ff2fe50c966a3f3dace2bc241bf604c42b6c84d75142f0a640e0b516724d20c7`. Bunlar sentetik fixture’dır, gerçek görev yürütme kanıtı değildir.

## Açık platform sınırları

Birinci taraf [platform kütüphanesi](../../crates/eylema-platform/src/lib.rs) yalnız birinci taraf contracts paketine bağlıdır. macOS CommonCrypto `CC_SHA256` hizmetini libSystem üzerinden (CC_LONG SDK’da u32), Security framework `SecRandomCopyBytes` hizmetini null/kSecRandomDefault ile kullanır. Gerçek SDK bildirimleri incelendi. Dar unsafe extern çağrıları slice/array uzunluğunu, pointer ömrünü ve dönüş kontrollerini kaydeder; contracts kütüphanesinde forbid(unsafe_code) korunur. Native SHA girdisi 4 MiB ile sınırlıdır; hizmet hatası veya desteklenmeyen native hedef açık hata döndürür. Kendi SHA algoritmamız, zaman/sayaç entropy yedeği, kopyalanmış SDK uygulaması veya üçüncü taraf Cargo bağımlılığı/build script yoktur.

ID’ler 16 byte işletim sistemi entropy’si, bilinen varlık öneki ve 32 küçük harfli hex karakter kullanır. Deterministik biçimlendirme birinci taraf Rust’tadır; entropy platform hizmetindedir. Örnekler hizmetin çalıştığını gösterir, mutlak benzersizlik veya kimlik doğrulama kanıtı değildir. Burada macOS dışı native hizmet yürütümü desteklenmez.

[Wasm ABI](../../crates/eylema-platform/src/wasm.rs) memory ile eyl_allocate_input, eyl_process, eyl_output_ptr ve eyl_output_len sağlar. Modlar 0/1/2/3: generic kanonik round-trip, tipli kayıt round-trip, onay payload’ı ve tür byte’ı + 16 entropy byte’ının biçimlendirilmesidir. Durum 0 başarı, 1 geçersiz sahipli buffer uzunluğu/lock, 2 geçersiz codec/tür/mod anlamındadır. Ayırma 1..4 MiB kabul eder ve girdi/çıktı durumunu temizler; geçersiz ayırma/işlem çıktıyı temizler. Host yalnız ayrılmış girdi aralığına yazabilir; tek instance üzerinde senkron işler ve sonraki değiştirici çağrıdan veya await’ten önce çıktıyı kopyalar. Çıktı bir sonraki değişikliğe kadar yaşar. Bu, güvenilen lineer bellek host sınırıdır; rastgele Wasm belleğine yazabilen düşmanca host’a karşı sandbox değildir.

Minimal [host bağı](../../platform/browser/codec-host.mjs) girdiyi ayırmadan önce kopyalar (Wasm belleğine alias olan view dahil), aralık/durum/çıktı boyutunu kontrol eder, çıktıyı hemen kopyalar ve digest/entropy’yi standart WebCrypto’ya devreder. Eksik/başarısız sağlayıcı hata verir; sessiz yedek yoktur. JavaScript codec, alan doğrulaması, ID biçimlendirmesi veya kriptografi uygulamaz. Tarayıcı secure-context/API kullanılabilirliği host önkoşuludur; Node testi gerçek tarayıcı kabulü sağlamaz.

Birincil API kaynakları: [Apple CommonCrypto](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/CC_SHA256.3cc.html), [Apple rastgelelik](https://developer.apple.com/documentation/security/randomization-services), [WebCrypto Recommendation](https://www.w3.org/TR/2017/REC-WebCryptoAPI-20170126/). İşletim sistemi SDK’sı/tarayıcı motorları bağımlılık olarak kalır.

## Çalıştırılan kanıt ve tekrar üretim

Yerel macOS 27.0.1 ARM64 / Apple M4 Pro 24 GiB, Rust 1.99.0, Python 3.14.8, Node v26.11.1. Node yalnız geliştirme aracıdır ve devguide’da ayrı kayıtlıdır; npm bağımlılığı kurulmaz. Mevcut ortamdan gelen RUST_LOG, değişmeyen ortam koruması için açıkça çıkarılır.

```sh
PATH="$HOME/.cargo/bin:$PATH" env -u RUST_LOG .venv/bin/python tools/check_rust.py
PATH="$HOME/.cargo/bin:$PATH" env -u RUST_LOG node tools/check_codec_platform.mjs
```

Passed: değişmeyen Rust fmt/clippy, birinci taraf offline/locked workspace envanteri, native test/build, seçilen contracts/platform Wasm derlemesi. Sekiz codec regresyon testi tüm tipli kayıtları/hata kodlarını, her konumda kesilmeyi, anahtar/UTF-8/sürüm/scalar/sınırları, bilinmeyen/eksik/daraltılan alanları, beyan bağındaki kaymayı, kanonik kümeleri ve taşıma/etki/sıra ayrımını denetler. Dört platform testi gerçek CommonCrypto empty/abc referanslarını ve gerçek Security ID’lerini içerir; enjekte edilen hata testleri yalnız hata iletimini sınar, platform yürütme kanıtı değildir.

İsteğe bağlı [karşılaştırma aracı](../../tools/check_codec_platform.mjs) önce değişmeyen Rust koruma/kontrolünü çalıştırır; ardından Rust Wasm modülünü Node v26.11.1 içinde gerçekten başlatır ve 33 kabul, 30 ret native fixture’ını karşılaştırır. Kabul edilen çıktı byte’ları ve gerçek Node WebCrypto SHA değerleri, gerçek macOS CommonCrypto değerleriyle eşleşir; sabit referans byte/digest’leri, sınırlar, 500 tekrarlı çağrı, girdi alias’ı, kopyalanan çıktının ömrü, geçersiz ABI uzunluk/modları ve eksik/başarısız crypto yolları geçer. G09 Node kanıtı yereldir; değişmeyen hosted CI Rust kontrolü yalnız native yürütümü ve seçilen Wasm derlemesini kanıtlar. Tarayıcı yürütümü, görev executor/admission, gerçek onay, service/journal, GPU/model/eğitim ve performans kabulü: not_run, sonraki Goal’lere atanmıştır.

## G28 ayrı artifact hash akışı

G28 aynı envanterdeki CommonCrypto sağlayıcısına kendi Sha256Stream arayüzünü ekler. Mevcut bütün wire/browser hash sınırları ve ABI korunur. Ayrı stream başlatılmış context'ini sahiplenir, toplam2GiB/parça1MiB sınırı koyar, hatada zehirler ve finalization ile tüketilir. Kurulu SDK/C/Rust layout ve bağımsız native vektörler geçti; gerçek model/büyük artifact veya yeni browser/Wasm çalışmasını kanıtlamaz. API, bellek ve güven sınırları için [ağırlık baytlarının yüklenmesine](weight-loader.md) bakın.
