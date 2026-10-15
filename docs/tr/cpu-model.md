# Sınırlı kendi CPU model yürütmemiz

[English](../en/cpu-model.md). Ayrı incelemeli skaler iş sayımı düzeltmesinden sonra düzeltilmiş kaynakta G30 sınırlı önceden eğitilmiş model kabulü aşağıdaki cihazda geçti. Son kanıt incelemesi, güncel teslim CI’ı ve koordineli main kabulü bekliyor.

`eylema-model::cpu` API'si yalnız ayrı kimlikli Qwen CPU manifestini kabul eder. Tam311tensor BF16 düzenini, değişmez G28 snapshot'ını ve özgün G29 tokenizer ebeveynini bağlar. Saklanan her skaler sonlu olmalıdır; fiziksel `lm_head.weight` embedding baytlarıyla eşit olmalıdır. Model tek snapshot ve tokenizer'ı ödünç alır; oturumlar yalnız sınırlı görünüm metaverisini kopyalar ve yeni KV/çalışma alanı tutar. G23 operatörleri ve genel unsupported-local ataması değişmez.

`qwen_layout`, mevcut yükleyiciye kesin düzeni verir. `CpuModel::admit`, `begin`, `CpuSession::prefill` ve `decode`, kabul ve sayısal yürütmeyi sağlar. `CpuLogits::values` ve `last_layer_hidden` sayısal teşhis verir; kabul edilmiş rol yargısı değildir. `max_new_tokens`, `generate` ile örneklenen teklifleri sınırlar ve prompt kapasitesi ayırır. Doğrudan `decode` sayısal girdi ekleyip yalnız logit döndürür; context/iş/son süre sınırları geçerlidir ve üretilmiş metin hakkı vermez. Model, tokenizer ve manifest ödünçleri canlı kalmalıdır. Başarısız hesaplama veya yinelenen prefill, kısmen eklenmiş cache dahil oturumu geçersiz kılar.

Çağıran context1–64, generation1–8, en fazla2560MiB mantıksal yerleşik içerik,80milyar iş birimi ve600saniyeyi aşmayan monoton son süre verir. Bilinen tahsis/okumalardan önce ve4096ücretlendirilmiş birimi aşmayan bloklarda sınırlar denetlenir. Ortak kontrol iptal, son süre ve atomik birikimli işi izler; limitler yalnız daralır. Yeniden kullanım ilerlemeyi sıfırlamaz. Bağımsız eşzamanlı işler bağımsız kontrol kullanır. Artifact kapasiteleri, muhafazakâr128MiB tokenizer/ön uç payı,4MiB yükleyici payı,128KiB metaveri payı, tam KV/çalışma alanı ve dışarı verilen logit tamponları muhafazakâr sayılır. Kabul işi ayrıca gösterilir; her oturum/teklif kendi kontrolünün işini raporlar. Tepe mantıksal rezervasyon; ölçülen RSS, allocator ek yükü, OS belleği ve throughput'tan ayrıdır. Genel iş zamanlayıcısı veya güvenli silme sözü yoktur.

Sayısal iş, sınırlı bloktan önce her aritmetik/aşkın/sonluluk-veya-karşılaştırma/okuma-dönüştürme işlemini sayar: RMS6n+4, residual toplama2n, head başına p öneki/w genişliği için attention p*(2w+3)+(6p+1)+w*(2p+1) birimdir. Sentetik testte de okuma dönüşümü muhafazakâr ücretlendirilir. Tam sınır/bir eksik testleri gerçek çekirdekleri kullanır; bütçesi yetmeyen blok işlemlerinden önce reddedilir. Sabit kontrol/indis takibi bu skaler metrik dışında kalır.

F32 aritmetiği sıralı skaler indirgeme, tam alan RMS/softmax, head başına Q/K normalizasyonu, yarım-parçalı RoPE, bitişik GQA, iki residual, SwiGLU ve bağlı logit kullanır. Query genişliği2048, hidden genişliği1024'tür. Küçük donmuş Decimal80 vektörleri, head128dahil1218son-durum/logit/KV değerini kapsar. Tüm BF16 desenleri, context/kaynak hataları, önek cache tutarlılığı ve gerçek hesap ortası iptal için yerel regresyon kapsamı vardır. Sentetik testler önceden eğitilmiş model kalitesi veya performansını kanıtlamaz.

`generate`, ayrı reasoning, planning, specialist ve critic istek türlerini kabul eder. Critic, sınırlı sabit sıralı JSON candidate/evidence nesnesini sıradan tipli user biçimlemesinden alır. Her çağrı yeni oturum başlatır. Çıktı türleri ve değişmez makbuzlar kesin rol arayüzünü, CPU/tokenizer/artifact kimliklerini, sayısal profili, token sayılarını, işi, mantıksal tepeyi ve EOS/token-limit sonucunu bağlar. Metin **doğrulanmamış tekliftir**; kalibre hüküm, skor, araç eylemi veya izin değildir. Boş, geçersiz UTF8ve kontrol işaretli çıktı reddedilir. Debug yalnız tür/sayı/açık kimlik özetleri içerir; prompt veya üretilmiş metin içermez.

Ek `encode_prompt_controlled`/`decode_controlled` yolları eski G29 sonuçlarını korur; normalizasyon, BPE ve decode içinde iptali taşır. G29'un4096token girdi sınırı ve128token rezervasyonu, küçük CPU prompt+generation denetiminden bağımsız kalır. Kütüphanede kesme, fallback, model indirme veya gizli motor yoktur.

## Geçmiş sınırlı yerel koşu

Aşağıdaki değişmez ölçümler onarım öncesi kaynak`7bd5debc4745ffda9abb36a81790b1c28511d5e5` sürümüne aittir. Bu sürümün iş sayacı yukarıdaki çekirdeklerde sonluluk koşulları, karekök ve bölmeleri eksik saydı; geçmiş iş toplamları eksiktir ve düzeltilmiş kaynak sözleşmesini kanıtlamaz. Yeniden hesaplanıp düzeltilmiş kaynakta çalıştırılmış gibi sunulmadılar. Sayısal/rol/durdurma gözlemleri geçmiş kanıttır. Özgün altı iş hakkı kullanıldı; düzeltilmiş kaynakta tekrar için açık ek insan yetkisi gerekir ve doğrulanmış ağırlık/referans yeniden kullanılır.

O kaynakta, AppleM4Pro/macOSARM64,24GiB birleşik RAM, Rust1.99.0 ve CPython3.14.8 üzerinde: tek açık1503300328bayt artifact162,19saniyede indirildi; tam dosya SHA256`f47f71177f32bcd101b7573ec9171e6a57f4f4d31148d38e382306f42996874b` doğrulandı. Kabul edilmiş G29 tokenizer/config dosyaları ayrıca hash denetiminden geçti. Hesap, provider, GPU veya eğitim motoru kullanılmadı.

Tek bağımsız F64 işi28katman durumunu ve151936logiti24,15saniyede hesapladı. Kendi Rust release kabul koşusu, snapshot/tokenizer/kabul hazırlığı dahil59,40saniyede tam altı aday işi çalıştırdı.180608hidden/logit karşılaştırmasının tamamı donmuş değer başına`absolute_error <= 0.001 + 0.0002*abs(reference)` sınırını sağladı; greedy token21806aynıydı. En büyük hidden mutlak hatası0,007344306 ve logit mutlak hatası0,000050857olarak raporlanır; bunlar değer başına bağıl sınırın yerine geçmez.

| Aday | Girdi/üretilen token | Doğrulanmamış çıktı UTF8 baytı | İş birimi |
|---|---:|---:|---:|
| Reasoning |21/8|22|50240061792|
| Planning |19/8|35|46645938084|
| Specialist |23/7|9|53835117641|
| Critic |30/8|26|66425102697|

Dört gerçek çıktı özeti birbirinden farklı; türler ve makbuzlar rollerle eşleşti. Kısa açık girdiler uzman veya Türkçe görev kalitesi değil, bağlantı/smoke davranışı kanıtıdır. Altıncı iş çalışma alanı hazırlığından sonra sayısal ilerleme gözledi, iptal etti, teklif döndürmedi ve cache yeniden kullanımını reddetti. Kabul işi3317825536iş birimi tüketti; kabul dahil altı işin toplamı222263133222ile320milyarın altında kaldı. Her iş80milyarın altında kaldı. Muhafazakâr tepe mantıksal içerik1658000646bayttı. İş başına örneklenen native süreç RSS1209253888–1538179072bayt aralığındaydı; F64 gözetmeni eşlenmiş sayfalar dahil1214775296bayt örneklenmiş tepe RSS gözledi. Örnekler genel sığma/throughput veya OS yalıtımı garantisi değildir.

Dört geçici gerçek-kaynak mutasyonu—iç içe GQA gruplama, yanlış yarım-dönüş işareti, eski önek anahtarları ve yok sayılan iptal—derlendi ve ilgili donmuş davranış testinde başarısız oldu. Her geri yükleme on iki CPU testini geçti; özgün kaynak ağacı tam korundu. Artifact gerektiren sıradan CI atlamaları açık kalır; yukarıdaki gerçek kabul ayrı yetkilendirilmiş yerel koşudur. Özel makbuzlar, loglar, model baytları ve referans dizileri Git dışında tutulur. GPU, eğitim ve daha büyük context yürütümü not_run kalır.

## Düzeltilmiş kaynağın gerçek kabulü

Bakımcı, skaler iş sayımı düzeltmesinden sonra altı ek yerel Rust işini açıkça onayladı; toplam12Rust aday işi, tek artifact indirme ve tek F64 referans işi kullanıldı. Kaynak`93bed57af6cbc8a33e619475e1bcedc88a06a0bd`, aynı altı işlik kabul programını bir kez çalıştırdı; yerelde hash’i yeniden doğrulanan artifact/tokenizer ve değişmemiş saklı F64 referans kullanıldı. Yeni referans hesabı, indirme veya provider/eğitim çağrısı yapılmadı. Geçmiş makbuzlar/loglar bayt düzeyinde korundu. Gözetmen kendi süreç grubunu,1200saniye son süreyi ve1MiB log sınırını kullandı; sonlandırma gerekmedi.

180608değer başına hidden/logit karşılaştırmasının tamamı özgün donmuş sınırı geçti; en büyük hatalar ve greedy21806yukarıdakiyle aynıydı. Dört rol çıktısının özeti geçmiş koşuyla aynı, birbirinden farklı; token/bayt sayıları da aynı kaldı. Düzeltilmiş iş makbuzları:

| Aday | İş birimi |
|---|---:|
| Sayısal |1795363620|
| Reasoning |50248076316|
| Planning |46653333550|
| Specialist |53843758391|
| Critic |66435991686|
| Etkili durdurma |3950042|

Kabul hazırlığı3317825536birim olarak kaldı. Düzeltilmiş toplam222298299141ile320milyarın; her iş80milyarın altında kaldı. Etkili durdurma sayısal ilerlemeyi gözledi, teklif döndürmedi ve cache yeniden kullanımını reddetti. Native kabul programı60,69saniye (gözetilen süreç61,24saniye) sürdü; örneklenmiş native RSS1206648832–1538965504bayt ile muhafazakâr mantıksal tepe1658000646bayt ayrı ölçümlerdir. Kullanılabilir bellek ön denetimi boş/inaktif/spekülatif OS sayfalarından4031856640bayt tahmin etti; bu kesin rezervasyon veya genel sığma garantisi değildir. Kanıt yalnız sabit küçük CPU sayısal/üretim/kontrol yoludur; model yararlılığı, kalibrasyon, büyük context, başka mimariler, GPU ve eğitim kanıtlanmış değildir.
