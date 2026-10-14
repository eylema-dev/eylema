# Sabitlenmiş model sözleşmesi — G27

G27, bildirilen model kökeni ve rol arayüzleri için gerçekten çalışan kendi Rust doğrulamamızı ekler. Ağırlık yüklemez, tokenizer/şablon/model çalıştırmaz, elde olmayan büyük artifact byte'larını doğrulamaz, eğitim/yetki/provider çağrısı yapmaz. [Model kataloğu](https://github.com/eylema-dev/eylema-models/blob/codex/g27-model-contract/catalog/qwen3-0.6b.v1.json) ve eşleşen yerel fixture seçilen Qwen adayını bağlar; aday kaydı yürütme kabulünden ayrıdır.

## Kapalı metaveri ve destek

`ModelManifest` doğrulamadan sonra değişmezdir. `model-profile.v1`, mevcut EYL codec'i ve değişmeyen görev/wire/journal sözleşmelerini kullanır. Kodlanmış metaveri en fazla32768bytes,1–64component,1–64operator ve1–128amaçlanan rol arayüzüdür. Düz ASCII adları, sıfır olmayan20byte kaynak commit/32byte hash'ler, kontrol edilen toplamla pozitif boyutlar, tam kaynak/haklar/component referansları ve sonlu boyut/iş sınırları denetlenir. Publisher/kaynak/lisans component'i sabitlenir; çözümlenmemiş veya yabancı koşullar reddedilir. Hak bildirimi proje erişimi veya bütün sonraki kullanımların hukuki değerlendirmesi değildir.

Katı decoder bilinmeyen yetki/destek alanlarını, yanlış tipleri, noncanonical/artık byte'ları, eksik/yinelenen referansları, sıfır kimlikleri ve sınır aşımını reddeder. Hash kökeni bildirilen `publisher_advertised` veya `audit_measured_small_bytes` olarak kalır; taşıma bunları doğrulanmış artifact kabiliyetine çeviremez. Somut katalog denetimi ölçülen küçük byte'ları kaydeder. Güvenilmeyen çağıran metaveride yanlış iddiada bulunabilir; bu constructor'lar güvenilir kanıt, kimlik, izin veya çalıştırılabilir destek vermez.

`check_assignment`, tam G46 rol arayüzünü eşleştirir: model kimliği rolden ayrıdır; schema/units/preprocessing/action/reset uyuşmalı, yinelenen profiller amaçlanan kaydı ezememelidir. CPU/Metal atamaları, `ModelExecutionNotImplemented` dahil sınırlı gerekçeyle `UnsupportedLocal` döndürür. `ExternalConfigured`, LocalOnly'de reddedilir; ExplicitExternal yalnız `ExternalConfigurationRequired` döndürür, dış çağrı/fallback/yetki vermez. G27'de Accepted/Executable constructor veya model motoru yoktur.

## Başlangıç adayı ve sınırlar

Qwen/Qwen3-0.6B kaynakc1899de289a04d12100db370d81485cdf75e47ca korunan adaydır. Config/license/tokenizer_config byte'ları ve4168byte UTF8 chat-template dilimi ölçüldü; model.safetensors ve tokenizer.json kimlikleri yalnız yayımlayıcının LFS SHA256 bildirimidir. İki büyük dosya indirilmedi/byte doğrulanmadı. Üçüncü taraf hakları Eylema lisansından bağımsızdır. Tam hash/boyut ve denetlenen kaynak/referans pinleri katalogdadır; üçüncü taraf referans uygulaması veya şablon üretime kopyalanmadı/çalıştırılmadı.

Bildirilen Qwen profili: bfloat16 safetensors/quantization yok,28layer, hidden1024/intermediate3072,16Qhead/8KVhead/head128, vocabulary151936 ve üst kaynak kapasitesi40960. Q projection width2048, hidden1024'ten geçerli biçimde farklıdır. Kontrol edilen grup başlık/projection/iş aritmetiği uyumsuz başlık/taşmayı reddeder. Dar planlanan batch1/context4096/en fazla128yeni token ölçülmüş kapasite/gecikme/kalite iddiası değildir. Kendi sabitlenmiş operator sözleşmemiz eager causal attention, FP32 RMS/softmax/position aritmetiği, default RoPE, Q/K head normalization, tied logits, KV reset ve greedy bounded generation tanımlar. Fiziksel yükleme/precision/tokenization/nümerik uyum sonraki G23/G28/G29/G30 kapılarıdır.

Dokuz isteğe bağlı rol perception, reasoning, planning, decision, policy, world_model, specialist, critic ve representation olarak ayrılır. STT/vision, TTS/text-domain ve embedding/reranking açık alan alt ayrımlarıdır. Qwen kataloğu yalnız reasoning/planning/specialist niyetini bildirir; tüm rollerin varsayılan ataması null'dır ve kabul edilmiş model kalitesi yoktur. Döngüler seçilen rolleri gerektirir; dokuzunun tamamını değil.

## Gerçek kanıt ve sınırlar

On beş yerel test gerçek katalog fixture çözümlemesini, tam kaynak/ağırlık metaverisini, canonical roundtrip'i, rol uyumsuzluğunu, katı alan/sınır/hakları ve yerelden dışa geçiş reddini kapsar. Core ile model kataloğu EYL byte'larının SHA256 değeri aynıdır:c087a033d4d193acd9912d0f555c99f14f9b968c9f308aca8acb52b07af85af2. On bir yalnız geliştirme Python testi offline katalog/rol/operator/hash/byte tutarlılığını ve hatalı girdiyi çalıştırır; Python üretim motoru değildir. Tam native gate, yerel Python kapıları, güncel CI, ayrı inceleme ve kabul edilmiş main kanıtları bağlı G27 kartı/PR'larında kaydedilir. Statik eşleşme ve constructor yürütmesi inference/kalite/GPU/eğitim/model byte doğrulaması/izin kanıtlamaz. Ortak kanonik JSON şemaları core'da kalır; validator/kaynak rolü/bağımlılık istisnası eklenmez.

Güven sınırları: incelenmiş denetim girdileri/kaynak alımı/saatler, kendi typed sözleşmemiz ve bildirilen profil anlamları. Sonraki yükleyici kullanımdan önce gerçek byte'ları doğrulamalı, gerekli her operator/backend/tokenizer'ı ayrıca kanıtlamalıdır. Bilinmeyen/desteklenmeyen profil fallback olmadan reddedilir. Withdrawal/collect/train/disclosure/export izinleri ayrı proje verisi kapılarında kalır.

Birincil kaynaklar: [sabit publisher dosyaları](https://huggingface.co/Qwen/Qwen3-0.6B/tree/c1899de289a04d12100db370d81485cdf75e47ca), [denetlenen referans commit](https://github.com/huggingface/transformers/tree/0720e206c6ba28887e4d60ef60a6a089f6c1cc76). Referans yalnız okundu/hash denetlendi; Transformers/PyTorch kurulmadı/çalıştırılmadı.

Kaynak precision BF16, planlanan FP32 CPU referansı ve FP16 Metal yürütme/dönüştürme profillerinden ayrıdır. Dönüştürme ve nümerik kabul sonraki kapılarda kalır; bu katalog özgün hedef yükümlülüklerinin yerini almaz.

Ayrı inceleme onarımı tam integer/identifier kabulünü, sabit ölçülmüş şablon kimliği/uzunluğunu, generic ve tüm layer Qwen extent taşma kontrollerini, açık tam operator kümesini ekledi. Semantik RED→GREEN regresyonları dört bulguyu doğrular; pozitif runtime desteği veya gerçek kaynak tahmini verilmez.

Onarım2, birleşik token sayısı × Q projection aktivasyon genişliğini taşma sınırının hemen altı ve sınırdaki regresyonla denetler; bu yapısal boyut kontrolü bellek tahmini veya çıkarım kanıtı değildir.
