# Eylema ilk sürüm yol haritası

Sürüm **0.2.0**, 11 Ekim 2026. [İngilizce](ROADMAP.md) kaynak dildir. Hedef **v0.1.0 deneysel** sürümdür; üretime hazır evrensel framework iddiası değildir. Aşamalar tarih sözü değil kabul kapılarıdır. Bugün yalnızca depo belgeleri vardır; katkı altyapısı PR'ı birleştirilip `main` üzerinde doğrulanana kadar M0 öneri durumundadır.

## Seçilen ilk çalışan uygulama

Bakımcı görev çekirdeği ve web arayüzünü seçti: görev oluşturma, çalıştırma, inceleme, durdurma, kaydetme ve yeniden açma. M1 deterministik yürütmeyi kurar, M2 kayıtlı görevle tarayıcı akışını tamamlar. [Bileşen önerisi ve kabul senaryoları](docs/tr/project-structure.md) bu hedefi uygulamadan önce incelenebilir kılar. İlk kanıt model veya GPU gerektirmez.

Sonraki deneysel sürüm aynı uygulamayı küçük 3B yerleştirme ortamı, öğrenilmiş politika/dünya dinamiği, desteklenen tek yerel dil modeli, izinli öğrenme ve geliştirme takımının PR akışıyla genişletir. Her yeteneğin kendi kabul koşulu vardır; ilk kanıtın seçilmesi sonraki yeteneklerin hazır olduğu anlamına gelmez.

İlk sürüm bütün GPU veya modelleri değil, belgelenmiş Linux cihazı ve tarayıcı/cihaz birleşimini destekler. Atama aşaması ikinci ölçülmüş donanım profili gerektirir. Sayısal limitlerden önce gerçek bellek, gecikme/kare süresi ve kalite başlangıçlarını ölçün; donanım ve iş yükü seçilmeden rakam uydurmayın.

| ID | Sürüm kapısı | Kapsam | Kabul kanıtı | Bağımlılık |
|---|---|---|---|---|
| M0 | Katkı temeli | Eylemci kuralları, iki dil, issue/PR şablonları, salt okunur CI | Doğrulayıcılar olumsuz örnekleri reddeder; PR ve main CI gözlenir; bakımcı incelemesi/dal kuralı durumu açıktır | Yok |
| M1 | `v0.1.0-alpha.1` | Kendi Rust çalışma alanı, tipli görev/durum/eylem sözleşmeleri, deterministik yürütücü | Sabit toolchain; harici üretim Cargo bağımlılığı yok; Linux/Wasm derleme; Linux birim/kurtarma testleri; sınırlar ve yetki retleri; yeni Rust kaynağı kabulünden önce bağlı CI | M0 |
| M2 | `v0.1.0-alpha.2` | İlk çalışan görev konsolu: kendi web arayüzü, yerel sunucu ve kalıcı durum | Oluştur, çalıştır, incele, durdur, kaydet, yeniden başlat/aç; yinelenen istek; yetki reddi; gerçek dışı durdurma onayı yok; ağ/kimlik/TLS sınırı belgeli | M1 |
| M3 | `v0.1.0-alpha.3` | Tarayıcı GPU sınırı ve küçük 3B sahne | Adlandırılmış tarayıcı/sürücüde gerçek WebGPU; cihaz kaybı ve desteklenmeyen yol hataları; sınırlı bellek/iş planlama; yükte kare süreleri; host bağları ve WGSL envanteri | M2 |
| M4 | `v0.1.0-beta.1` | CPU tensör referansları, öğrenilmiş karar politikası, küçük dünya dinamiği modeli | Bildirilmiş toleransta CPU/GPU eşliği; ayrılmış görevlerde kapalı döngü başarı; tahmin hatası; kısıtlara uyum; deterministik başlangıcı geçme veya geçememe sonucu açık | M3 |
| M5 | `v0.1.0-beta.2` | Kendi motorumuzda tek sabit küçük yerel dil modeli | Ağırlık/tokenizer/işlem/nicemleme doğrulaması; yükleme/ilk sonuç/sıcak ölçümler; ayrılmış görev kalitesi; ortak sahne/politika/LLM bütçeleri; desteklenmeyen modelde açık hata; gizli bulut fallback yok | M4 |
| M6 | `v0.1.0-rc.1` | Sürümlü model atama ve izinli öğrenme döngüsü | İki ölçülmüş profil; kabul edilmiş takım manifesti; canary/geri dönüş; veri izni/köken/tekilleştirme/holdout; sabit dış Unsloth tarifi; adayın kendi motorumuzda yeniden doğrulanması | M5 |
| M7 | `v0.1.0` | İhtiyaçtan PR'a geliştirme protokolü ve deneysel sürüm | Girdi, plan, uygulama, kontroller ve bağımsız incelemeden geçen gerçek görev; aynı sözleşmede iki yapılandırılmış sağlayıcı adaptörü; yinelenmeyen PR tekrarları; sınırlar, lisans ve yeniden üretilebilir sürüm kanıtı | M6 |

## İlk uygulanabilir iş listesi

Bu kimlikler plan referansıdır; açılmış GitHub issue'su değildir. Uygulamadan önce bir satırı kabul testleri olan sınırlı issue'ya dönüştürün.

| ID | Sonraki iş | Tamamlanma koşulu |
|---|---|---|
| EYL-001 | Lisans ve katkı koşullarını seç | Bakımcı kararı ve köken kuralları kaydedilmiş; örtük katkıcı anlaşması yok |
| EYL-002 | İlk iş yükü ve donanım matrisini sabitle | Linux, tarayıcı ve ikinci atama profili ile yeniden üretilebilir sahne/görev verisi belgeli; erişilemeyen donanım `not_run` kalır |
| EYL-003 | Kendi Rust çalışma alanı ve CI'ını kur | Kesin Rust sürümü ve lockfile; Linux/Wasm derleme ve gerçek Linux testleri; harici runtime crate'i olmadığını gösteren envanter; belge aşaması kaynak engeli yerine gerçek Rust kapıları |
| EYL-004 | Görev durum makinesini uygula | Başarı, hata, yetki reddi, zaman aşımı, yinelenen teslim ve iptal testli; dış etkiler için tam bir kez garantisi iddiası yok |
| EYL-005 | Kalıcı yerel görev günlüğünü uygula | Kesilmiş kayıt, çökme/yeniden başlatma ve uyumsuz biçim testli; sürüm ve kurtarma davranışı açık |
| EYL-006 | İlk web görev ekranını yap | Oluştur/çalıştır/incele/durdur/kaydet/yeniden aç akışı tarayıcıda sınırlı yetki ve görünür hatalarla gösterilmiş |
| EYL-007 | WebGPU kaynak sınırı prototipi | Tek hesaplama ve 3B sahne gerçek donanımda çalışmış; CPU referansı, desteklenmeyen yol ve cihaz kaybı raporlanmış |
| EYL-008 | İlk model sözleşmelerini seç | Politika/dinamik/LLM mimarileri işlem kapsamı, dağıtım hakları ve ölçülmüş kaynak sınırlarına göre seçilmiş; seçim destek kanıtı değildir |

## v0.1.0 sürüm kontrol listesi

- Her aşamanın zorunlu kontrolleri aynı sürüm adayı için güncel kanıta sahiptir; atlanan zorunlu donanım kontrolleri ilgili destek iddiasını engeller.
- Tanımlanan profiller için kalite eşikleri, p95/p99 gecikme/kare süresi, bellek ve başlangıç limitleri belirlenmiş ve karşılanmıştır. Kazançlar kadar başarısızlıklar raporlanır.
- HITL/HOTL/HIC sözleşmeleri, yetki geri alma, müdahale ve geri dönüş sınırları denenmiştir. Model çıktısı yetki genişletemez.
- EN/TR rehberler ilk uygulamayı yeniden üretir; desteklenen/desteklenmeyen hedefler açıktır. API/kod/şema sürümleri uyumludur.
- Dağıtım lisansı, model/veri hakları, toolchain/sürücü/tarayıcı envanteri ve sınırlar kayıtlıdır. Kernel yeniden yazımı, üretim kriptografisi veya platform yetkisini aşan donanım erişimi ima edilmez.
- Sürümü bakımcı kabul eder. CI tek başına model etkinleştiremez, merge veya sürüm yayını yapamaz.

## İlk sürüm dışında bırakılanlar

Genel CAD çekirdeği, bütün model mimarileri, dağıtık eğitim, yalnız Rust'tan oluşan Linux çekirdeği, her GPU sürücüsü, otomatik satın alma, gözetimsiz ayrıcalıklı kendi kendini değiştirme ve tüm üretim web özellikleri sonraki kararlardır. Yol haritası uzun vadeli mimariyi korurken önce sınırlı tek uçtan uca uygulamayı kanıtlar.
