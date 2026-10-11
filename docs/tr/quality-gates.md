# Depo kalite kontrolleri

Sürüm **0.1.0**. [İngilizce](../en/quality-gates.md).

## Bugün çalışan kapı

[CI](../../.github/workflows/ci.yml) içindeki `repository-checks` işi PR'larda ve `main` push'larında çalışır. GitHub'ın sağladığı runner, salt okunur token, commit'e sabit eylemler ve kalıcı tutulmayan checkout kimliği kullanır. Sırlar, model API'si veya self-hosted donanım kullanmaz. PR açıklaması olay dosyasından veri olarak okunur. İş akışı ayrıcalıklı PR tetikleyicisi veya yayın işlemi içermez.

| Kontrol | Hangi durumda başarısız olur | Neyi kanıtlamaz |
|---|---|---|
| Depo dosyaları | Sembolik bağlantı, geçersiz JSON/YAML, kırık/depo dışına çıkan yerel Markdown hedefi, özel bilgisayar yolu | Dış bağlantıların sürekli erişimi; kapsamlı sır taraması |
| Şema örnekleri | Geçersiz Draft 2020-12 şema, uzak referans, geçersiz istek örneği, şema regresyonu | Kimlik, onay, referans verilen nesnenin varlığı veya doğru model yürütmesi |
| EN/TR sözleşmeleri | Eksik/eski çeviri özeti, API işlem/kod kimliği farkı, yol haritası kimliği farkı | Anlamsal çeviri doğruluğu |
| GitHub yapılandırması | Eksik girdi alanı, bozuk form, eksik PR bölümü, sabitlenmemiş/beklenmeyen action, ayrıcalıklı iş akışı ayarı | GitHub'ın şablonu sunucuda doğru çizmesi veya dal kuralını uygulaması |
| Gerçek PR açıklaması | Eksik, yinelenmiş veya yalnız yer tutucu içeren zorunlu bölüm | Kanıtın doğruluğu, inceleyici bağımsızlığı veya yetki |
| Kaynak aşaması sınırı | Yalnız belge kontrolleri varken Rust/Cargo kaynağı ekleme | Rust derleme/yürütme; EYL-003 bu sınırı gerçek kapılarla değiştirmelidir |
| Boşluk ve doğrulayıcı testleri | Git boşluk hatası, başarısız regresyon testi | Uygulama davranışı |

PR açıklama kontrolü PR olayına bağlıdır; push olayında doğrulanacak PR gövdesi yoktur. CI gerçek PR head SHA'sını ve checkout revizyonunu yazar; PR çalışması GitHub'ın sentetik birleşim commit'ini kontrol edebilir. Eski commit'teki yeşil sonuç güncel kanıt değildir. PR açıklaması düzenlenince kontrol yeniden tetiklenir.

## Bakımcı kapıları

Gereksinimler, üretim bağımlılık politikası, bağımsız inceleme, lisans, API/denetim anlamı ve sürüm yetkisi bakımcı kararlarıdır. `CODEOWNERS` inceleme yönlendirmesidir. Bu iş akışı zorunlu kontrol kuralını veya dal korumasını etkinleştirmez. Bakımcı çalışmayı gözlemleyip inceleme gereksinimini belirledikten sonra `repository-checks` işini zorunlu yapabilir. Fork kontrolünü geçirmek için geniş yetki vermeyin.

Doğrulayıcı/politika değişiklikleri kabul edilmiş başlangıç politikasıyla karşılaştırılmalıdır; değiştirilmiş doğrulayıcıyı çalıştıran PR kendi gevşetmesini onaylayamaz. Ayrı inceleme kesin revizyonu belirtmelidir. Böyle bir inceleme yoksa beklemede raporlanır.

## Sonraki çalıştırılabilir kapılar

EYL-003, Rust kaynağı kabulünden önce sabit toolchain, kök workspace/lockfile, format, clippy, davranış testleri, bağımlılık envanteri ve Linux/Wasm derlemesini ekler. Native Linux yürütmesi derlemeden ayrı raporlanır. Tarayıcı/GPU, performans, kalıcılık, politika/dünya modeli, dil modeli ve eğitim kabulleri kendi yol haritası aşamalarında kontrollü donanımla eklenir. Public PR kodu workflow değiştirerek özel veri veya güvenilen dağıtım runner'ına erişemez.

## Kontrolleri güncelleme

Düzeltilen hatanın reddedildiğini gösteren regresyon testleri ekleyin. Standart [katkı komutlarını](../../CONTRIBUTING.tr.md) çalıştırın. Çeviri özetlerini iki dili inceledikten sonra yenileyin. Depodaki [eski doğrulama kaydı](../../validation-report.json) tarihsel kalır; güncel commit'in kanıtı güncel CI loglarıdır.

Kaynaklar: [GitHub workflow sözdizimi](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax), [güvenli kullanım](https://docs.github.com/en/actions/reference/security/secure-use), [issue form sözdizimi](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-issue-forms).
