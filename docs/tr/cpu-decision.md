# G57 sınırlı CPU karar kabulü

Durum: yetkilendirilen tek yerel artifact/referans kabul çağrısı geçti. Bu kayıt yalnız sabitlenmiş artifact ve sabit girdiler için kanıttır. Bağımsız son-commit incelemeleri, güncel exact-head CI ve koordineli merge bitene kadar G57 teslimi beklemededir.

## Girdiler ve oracle

Sabitlenmiş `Qwen/Qwen3-Reranker-0.6B` kaynak revizyonu `e61197ed45024b0ed8a2d74b80b4d909f1255473` idi. Kabul edilen BF16 artifact 1.191.588.280 bayt ve SHA-256 değeri `27cd75a405b9c1b46b59abfd88aaa209e6fed2a1972cde9b70e7659537c5e65b` idi. Kendi yükleyicimizin 310 tensor düzeni ve tam dosya kimliği kontrolleri geçti. Sınırlı tokenizer girdisi, daha önce doğrulanmış sabit `tokenizer.json` dosyasıydı; içeriği ve token satırları burada yayımlanmıyor.

Geliştirme amaçlı F64 oracle, bağımsız olarak Rust ile yazıldı ve sabit Decimal80 fixture’larına karşı denetlendi. Kaynak Rust 1.99.0 `-C opt-level=3` ile derlendi; oracle’ın iki sentetik testi ve `--self-test` geçti. Optimize edilmemiş ilk derlemeyle yapılan önceki deneme 600 saniye sınırını aşıp başarısız kaydedildi; yetkili optimize tekrardan önce kaynak ve girdiler değişmedi. Ölçülen referans özeti `147e416f1dca6006b03176db99f00c1553f7815da3ba6fc60ff31c11bb1f67d5` idi.

## Native sonuçlar

İlk yetkili kabul denemesi, aday işlerine başlamadan 600 saniyelik sınırı aştı. Kullanıcı bir sınırlı devam çağrısını onayladıktan sonra bu çevrimdışı, kilitli release komutu çalıştı ve yok sayılmış gerçek-varlık testini 487,46 saniyede geçti. Bağlam sınırı 128 olan dört sabit token satırı karşılaştırıldı. Katman gizil durumundaki en yüksek mutlak fark `0,0005589494`; yes/no head logitlerindeki en yüksek mutlak fark `0,0000113805` idi. İkisi de donmuş değer başına `0,001 + 0,0002 * abs(reference)` sınırının içindeydi.

İki sorulu/dört çiftli batch, sabit İngilizce ve Türkçe smoke sorularında önceden belirlenen pozitif seçeneği döndürdü. Soru/seçenek sırası ters çevrildiğinde eşleşen çift sonuçları değişmedi. İptal işi hesaplama sırasında durdu, tamamlanmış batch sonucu vermedi ve kısmi öneri yayımlamadı. Tam bağlam sınırı başarılı; bir alt sınırdaki preflight ise ileri hesaplamadan reddedildi. Bozuk girdi ve sınırlı kaynak negatif kontrolleri geçti.

Toplam sayaçlı iş `956151402665` birim oldu. Mantıksal tepe ayırma `1361720300` bayt; ayrıca örneklenen süreç RSS değeri `905183232` bayt idi. Bu ölçümler yalnız bu çağrının çalıştığı host’u tanımlar; yalıtım garantisi veya evrensel kapasite/performans iddiası değildir.

## Sınırlar ve teslim kapıları

Bu küçük sabit smoke kümesi kalibre edilmiş ilgi kalitesini, geniş Türkçe kaliteyi, istatistiksel görev başarısını, genel context kapasitesini, throughput’u, GPU/Metal yürütümünü, eğitimi, üretim yetkisini veya proje verisini toplama/açıklama/aktarma iznini kanıtlamaz. Skorlar kalibre edilmemiş öneri verisi olarak kalır ve etkili eylemleri yetkilendiremez. İlk zaman aşımı özel ledger’da korunur. Model ağırlıkları, tokenizer içeriği, promptlar, token ID’leri ve özel alındılar depoya eklenmemiştir.

G57 kabul edilmeden önce kaynak, katalog ve rehberin son commit’lerinde bağımsız inceleme, güncel exact-head CI, bağımlılık sırasına göre merge ve merge sonrası main doğrulaması gerekir.
