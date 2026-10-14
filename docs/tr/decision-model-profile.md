# Bildirilen karar/skor modeli profili

G56, mevcut kendi Rust model sözleşmemize dar metin ilgililik karar/skor adayı kaydeder. Gerçek Rust testleri gerçek EYL metaveri fixture'ını çözer; Python yalnız offline geliştirme denetimi ve sentetik referans oracle'ıdır. Bu dilim ağırlık, tokenizer/template/model çıkarımı, yerel skor kernel'i, kalibre güven veya görev kalitesi kabulü sağlamaz.

## Aday ve köken

Seçilen aday [Qwen3-Reranker-0.6B](https://huggingface.co/Qwen/Qwen3-Reranker-0.6B/tree/e61197ed45024b0ed8a2d74b80b4d909f1255473), kaynak e61197ed45024b0ed8a2d74b80b4d909f1255473; 14 Ekim 2026'da denetlendi. Yayıncı metaverisi 595.776.512 BF16 parametre ve 1.191.588.280 byte model.safetensors bildirir; bunlar ağırlık-byte denetimi veya ölçülen resident bellek değildir. Hidden1024, intermediate3072,28katman,Q16/KV8/head128,vocab151669 ve positional capacity40960 config bildirimidir. Hedef batch1/context4096 daha dardır; skor metin üretmez, max_new_tokens1 yalnız ayrılmış pozitif metaveri üst sınırıdır. KaynakBF16, gelecekCPUFP32 ve gelecekMetalFP16 farklı profildir; doğrulanmış dönüşüm değildir.

Ölçülen public config/README/tokenizer_config/vocab byte'ları sabitlenir. Vocab yes9693/no2152 içerir; tam tokenizer.json ve ağırlıklar yoktur, LFS hash'leri yalnız yayıncı bildirimidir. Sabit ağaçta LICENSE dosyası yoktur; haklar ölçülen README yayıncı Apache-2.0 bildirimine bağlanır; uydurulmuş LICENSE byte'ı, proje erişimi veya dağıtım yetkisi değildir. Haklar artifact özelinde kalır.

Tarihli karşılaştırma [Jina reranker v3](https://huggingface.co/jinaai/jina-reranker-v3/tree/d7d7e73b6ea138ced340b83865931b5dfb6c97aa) metaverisini de gözledi: son değiştirme 10 Ağustos 2026,595.836.352 parametre ve cc-by-nc-4.0 lisans tanımlayıcısı; Qwen metaverisi son değiştirme16Nisan2026. Bu kaynak metaverisidir; aynı cihaz benchmark'ı veya hukuki uyumluluk sonucu değildir. Jina'nın tam head/operator/yerel/hak profili burada kabul edilmez. İlk Qwen adayı sabit dense backbone açıklamasını ve açık skor head'ini kullanır; en yeni/büyük/global en iyi veya yerel destek iddiası yapılmaz. Gelecek tarihli keşif ve bağımsız kalite/kaynak kabulü G65/G80–G82'de kalır.

## Sabit rol ve skor semantiği

Tek isteğe bağlı Decision arayüzü decision.query-options.v1 → decision.score-choice.v1; UTF8-pair girdi, uncalibrated-score-micros çıktı, proposal-only etki ve reset-per-pair durumdur. Dokuz sabit rol/default-null atama korunur; skor modeli genel eylem policy veya dünya modeli değildir.

Kendi bileşenimiz tam236byte rol şablonunu ve hash'ini tanımlar: c91af212ad3548b6a9f87c8b54901bb171e2f596e7d61f666634e44fc690a637. Yayıncı örnek preprocessing'inden farklıdır; G57 bu tam prompt için gerçek kaliteyi değerlendirmelidir. Alanlar bir kez literal yerleştirilir, reserved control marker reddedilir, tokenization sonrası context sınırı uygulanır ve sessiz truncation olmaz; bunlar bildirilen şartlardır, şu anda yürütülen tokenizer/renderer değildir. Özel muhakeme toplanmaz. Yalnız okunan referans açıklaması Transformers, SentenceTransformers veya başka motoru kurmaz/çalıştırmaz.

Her çift için hedef head son nonpadding yes/no logits'i seçer. Ham çıktı yes-minus-no logit farkıdır; iki etiket sigmoid'i kalibre olmayan ilgililik skoru üretir. Sabit nokta micros ölçeği1.000.000/half-even yuvarlamadır. Seçim tüm aday skorlarını,750.000 tabanı ve100.000 farkı ister; eşitlik, düşük taban, küçük fark ve eksik skor açık abstain olur. Geçersiz/nonfinite/profil dışı girdi reddedilir. Sınırlar:4soru×4seçenek,16toplam çift,64byte kimlik ve sentetik referansta mutlak delta≤32. Bunlar sabit önerilen profil kapılarıdır; öğrenilmiş başarı olasılığı değildir.

Altı sabit referans vakası çoklu soru, eşitlik, düşük taban, küçük fark, eksik skor, sınırlı uç değer ve yayıncı bildirdiği7.625/−11.375 ham delta dönüşümünü kapsar. Scalar değerler standart Decimal80 ve elle belirlenen seçim sonuçlarıyla bağımsız sabitlendi; denetim bağımsız uygulanmış kararlı sigmoid biçimini kullanır. Yayıncı örnek çıkarımı tekrarlanmadı ve preprocessing kendi prompt'umuzdan farklıdır. Referans başarısı yalnız aritmetik/bildirim kanıtıdır.

## Gerçek kontroller ve sınırlar

Sekiz geliştirme testi gerçek JSON/EYL eşitliğini, değişmez head/kaynak/config/prompt/operator/referans bağlarını, tam integer tipini, sahte destek/yetki flag'lerini, duplicate/aşırı büyük/symlink JSON ve hatalı/sınırlı soru verisini denetler. Altı gerçek Rust fixture testi canonical metaveri çözümlemesi, bildirilen köken, tam rol/reset/birim, değişmeyen kaynak/hak/yük reddi ve kapalı yetki alanlarını çalıştırır. Yeni head/choice operator'ları CPU/Metal için UnsupportedLocal(Operators) kalır. LocalOnly dış rotayı reddeder; ExplicitExternal yalnız ExternalConfigurationRequired döndürür, çağrı yapmaz. Üretim Rust modülleri, kanonik şemalar ve EYL/task/journal arayüzleri değişmez.

Model/core fixture SHA256 076c625c334ee9e2619fcb6016acc2314a51288a9a22baa6dde4029962a87da2 (4517 byte) birebir aynıdır. Tam yerel/güncelCI/ayrı inceleme/kabul edilen-main kanıtı G56 kartı ve PR'larına bağlanır; hash eşleşmesi gerçek model, sayısal, bellek/performans veya yetki kabulü kanıtlamaz.

## Platform ve veri yönü

Eyleme'nin hedefi tam eylemci çalıştırma platformudur: insan kontrollü yeterliği kabul edilmiş büyük model geliştirmesi; mevcut/planlanan kaynaklarda gizlilik odaklı yerel açık ağırlık uzman işbirliği; kendi Rust harness/memory/RAG/model özelinde eğitim ve reçete/iyileştirme döngüleri. Yerel artifact ve türevler, gerçek ayrı collect/train/disclose/export hakları incelenmiş paylaşıma izin vermedikçe proje/kurum içinde özeldir. Anonim ekosistem deneyimi/reçeteleri hak, rıza ve artık yeniden kimliklendirme incelemesi gerektirir; kaynak/model skoru ve redaction tek başına yayın yetkisi vermez. Bu Goal yalnız public aday metaverisi ve kendi sentetik fixture'larımızı tüketir. Kullanıcı/kurum kaydı burada toplanmaz veya dışa çıkarılmaz.
