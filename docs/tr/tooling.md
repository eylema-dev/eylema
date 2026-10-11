# Araç ve bağımlılık envanteri

Sürüm **0.1.0**. [İngilizce](../en/tooling.md).

Üretim hedefi varsayılan olarak harici Cargo paketi kullanmayan kendi Rust bileşenlerimizdir. Depoda bugün üretim kodu yoktur. CI araçları uygulama bağımlılığına dönüşmez. Linux, tarayıcı, sürücüler, GPU arayüzleri ve model dosyaları Rust kullanınca ortadan kalkmayan açık platform girdileridir.

| Sınır | Araçlar | Amaç / sabitleme |
|---|---|---|
| Yalnız depo doğrulama | CPython 3.14, `jsonschema` 4.26.0, PyYAML 6.0.3 | Standart ayrıştırma/doğrulama; kesin sürümler ve bütün dosya özetleri [requirements-ci.txt](../../tools/requirements-ci.txt) içinde |
| Yalnız CI dolaylı paketleri | attrs 26.1.0, jsonschema-specifications 2025.9.1, referencing 0.37.0, rpds-py 2026.9.1 | Doğrudan paketlerle birlikte özetleri kilitli; uygulamaya dahil edilmez |
| GitHub checkout | actions/checkout v7.0.1, `3d3c42e5aac5ba805825da76410c181273ba90b1` | Resmî GitHub eylemi; erişim bilgileri kalıcı tutulmaz |
| Python sağlama | actions/setup-python v7.0.0, `5fda3b95a4ea91299a34e894583c3862153e4b97` | Resmî GitHub eylemi; minor 3.14, patch sürümü CI'da yazdırılır |
| Barındırılan ortam | GitHub `ubuntu-24.04`, Git | İmaj revizyonu değişebilir; yeniden üretimde runner/runtime sürümlerini kaydedin |
| Yalnız mevcut marka dosyalarını yeniden üretme | Node.js ve sharp | [Marka kılavuzu](../../assets/eylema-logo/README.tr.md) ilk üretimde sharp 0.35.5 kullanıldığını kaydeder. Main'den devralınan isteğe bağlı çizim aracı; CI tarafından kurulmaz, uygulamada kullanılmaz. Bu katkı değişikliği yeniden üretim bağımlılık kilidi sağlamaz |
| Planlanan üretim araçları | Rust/Cargo, Wasm hedef araçları | Kesin sürüm, hedef envanteri ve Rust kontrolleri EYL-003'te; bugün Rust testi iddiası yok |
| Planlanan eğitim ortamı | Unsloth ve Python/hızlandırıcı bağımlılıkları | M6'da ayrı sabit imaj/tarif; depo CI'ında kurulmaz veya kullanılmaz |

Kurulumu izole Python 3.14 ortamında `python -m pip install --require-hashes -r tools/requirements-ci.txt` ile tekrarlayın. Araç güncellemesinde sürüm/köken incelemesi, tam bağımlılık çözümü, PyPI dosya SHA-256 kayıtları, yeni kilitle test ve envanter güncellemesi gerekir. GitHub action güncellemesinde resmî sürüm etiketi tam commit SHA'sına çözümlenir. Otomatik bağımlılık güncelleyici kurulmadı.

Kendi bileşenlerimizi geliştirme politikası; incelenmemiş kriptografi yazmayı, belge aşamasında standart şema doğrulamayı yeniden yazmayı veya tamamen Rust işletim sistemi iddiasını gerekçelendirmez. Üretim bağımlılık sınırındaki değişiklikler mimari öneri ve bakımcı kararı gerektirir. Asgari tarayıcı host bağları ve shader kaynakları M3'te açıkça envantere girer.
