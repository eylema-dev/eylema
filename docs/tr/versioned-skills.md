# Sürümlü öz skill paketleri

G75, [türlü araçlar](typed-tools.md), [karar kanıtı](decision-evidence.md) ve [proje-verisi izinleri](project-data.md) üzerinde iki kapalı native yaprak skill'i yeniden kullanır. Rust, script, plugin, süreç, veritabanı motoru veya model yüklemez.

## Paket ve döngü sözleşmeleri

SkillDefinition, sabit skill kimliği/revizyonu, tam girdi/çıktı şemalı değişmez araç profili/revizyonu/işlemi, adaptör kimliği, isteğe bağlı bildirilen model profili/artifact/revizyonu/adaptörü ve bildirilen kabul referansı taşır. Ayrı kapalı skill/v1 metaverisi tam12alan kullanır; ek yetki alanı, iç içe işlem, bilinmeyen sürüm/handler, uyumsuz şema, geçersiz/sıfır model özeti veya revizyonu reddedilir. Kimlik/revizyon/özet eşitliği sabitlemedir; model uyumluluğu veya kabul edilmiş eğitim kanıtı değildir. Yalnız mevcut readTaskValue/addTaskValue handler'ları ve owned:native-counter:v1 adaptörü çalışır. Her model bağımlılığı veya diğer adaptör bulunamaz döner; sessiz fallback yoktur.

Düz SkillLoop, döngü kimliği/revizyonu,1–8sıralı proje kaynağı/skill/revizyon çağrısı ve1–8tekrar sabitler. Açılan çağrı toplamı≤8olmalı ve tek projede kalmalıdır. İç içe/özyinelemeli program bulunmaz. Çağıran somut ToolPlan sağlar; yükleyici her açılmış çağrıyı gerçek profil/revizyon/şema/işlemle eşler, ardından değişmeyen G10/G11 kabulü gerçek kapsam, yetki, durum revizyonu, girdi, onay ve kaynak bütçesini doğrular. Paket/referans/kabul etiketi bu hakları veremez.

Gerçek G63 Execute yetkili Artifact/Derived kayıtları kanonik paket baytlarını sağlar (≤4096bayt). Çözümleme mevcut sınırlı EYL codec ve round-trip eşitliğini kullanır. Kabul referansı bildirilen metaveridir. Mühürlü SkillWitness gerçekten yüklenen döngüyü, sıralı çağrıları, kaynak kimliklerini ve özgün put-makbuzu özetlerini bağlar; SkillEvidence bunu gerçek görev/yürütme/eylem gözlemi ve tam/kısmi/bilinmeyen sonuçla eşler. Put özeti bağımsız doğrulanmış ağırlık/içerik imzası olarak sunulmaz. Artifact kaydı/model kabulü ve gerçek native çıkarım ayrı kapılardır.

## Kontrol kaynağı gizliliği

Tanım, skaler argüman vermeden eylemi etkileyebilir. G54 soyu artık sıradan argüman kaynaklarının yanında≤8benzersiz kontrol kaynağı tutar. Kabul, her normal adım ve her sonuç/görev okuması tüm tutulmuş görev kontrollerini, kaynak atalarını ve mühürlü veri oturumlarını yeniden yetkilendirir. Devirde ortak kalıcı saat gözlemi tüm kontrol ve argümanları denetler; her eylem süresi kaynak/oturum süre sonuna sığmalıdır. Yabancı, aynı görünen depolar özgün canlı bağı değiştiremez.

Kontrol taşıyan plan her eylemde açık müdahale, gerçek kapsamlı denetleyici kullanılabilirliği ve G11 ayrılmış iptal olayları ister. Gerçek kapsamlı insan iptali, kaynak çekilmesi/süre sonundan sonra özel gövdeyi okumadan işi bitirebilir. Sonraki kaynak belirtmeyen eylemler geçmiş kontrol kaynaklarıyla sınırlı kalır. Tam plan/kontrol/döngü/sürüm/özet replay'i özgün tanığı korur; skill tanığını değiştirme/silme özel koşuyu literal diye yeniden etiketleyemez. Tamamlanmış ön ek etkileri durdurma veya çekmeyle geri alınmaz.

Özel türetilmiş paket ataları güncel amaç/süre sonu/çekme sınırlarını G63 üzerinden devralır. Execute kanıtı Collect/Train/external_disclosure/ecosystem_export hakkı üretmez. Uygun literal, sır olmayan metaveri, tam bildirilen soy, güvenilir alım/host/depolama/hash/saat ve özel journal önkoşuldur; otomatik DLP veya gerçek kişi kimliği iddiası yoktur. Dönen bayt geri çağrılamaz. Türlü tanık/kontrol soyu geçicidir; komut geçmişi olan host içe alınamaz, bilinmeyen kurtarma yalnız kapsamlı durum/sayı incelemesi verir. Eski ham journal tüketicileri özel kapıları aşmamalıdır.

## Ölçüm ve kabul kapsamı

G50 kıyaslanabilir iş yükü anahtarı, varsa gerçek skill yükleyici/döngü/kaynak/sürüm/put-özet sabitlemelerini içerir. Farklı skill soyu yalnız sayaç payload'ı eşleşiyor diye aynı tamamlanan dağılıma giremez. Mevcut karar süreleri iç türlü kabul/adım/terminal durumunu kapsar; paket yükleme/çözümleme bu süreden önce gelir ve tam skill başlangıç benchmark'ı değildir. Fiziksel bellek, model performansı, tam çok modelli döngü veya eğitim iddiası çıkmaz.

Native testler ekleme/okuma paketlerini iki farklı sınırlı döngü tanımında gerçekten kullanır; sayaç çıktısını ve değişmez replay'i doğrular. Hata testleri etki öncesi model/adaptör/şema/ek alan reddi, yanlış çağrı sırası, eksik kaynak kapsamı, yabancı veri oturumu/depo, özgün ve tutulmuş kontrol sınırları, türetilmiş ata çekme, yalnız Collect reddi, mühürlü oturum süresi, gerçek denetleyici yokluğu ve etkili kapsamlı durdurmayı kapsar. Taşınabilir testler kapalı codec ve sınırlı tekrar/çarpım/proje kurallarını kapsar. Kaynak ve saat/insan yetkisi örnekleri sentetiktir; miras alınan tam native testler mevcut kapsamını korur.

Yerel hedef ARM64 macOS/Rust1.99.0'dır; yeni fiziksel IO/süreç/provider/G40/model/GPU/eğitim/motor/bağımlılık denemesi yoktur. Tam AGENTS kontrolleri, güncel CI ve gerçek ayrı son-commit incelemesi zorunludur. Yalnız native API: genel iç içe akış, kalıcı türlü skill yeniden bağlama, gerçek kişi girişi, eğitilmiş model skill paketleme, veri seti/ihracı ve uçtan uca kullanıcı uygulaması sonraki kabullerde kalır.
