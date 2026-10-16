# Projeye özel kendi erişim indeksimiz

[İngilizce kaynak](../en/retrieval.md). G70, sınırlı ve birinci taraf Rust erişim kütüphanesi ile yerel yetki bağdaştırıcısını uygular. Embedding modeli çalıştırmaz ve yeni HTTP/UI hizmeti açmaz. Bu kabulün vektörleri herkese açık sentetik girdilerdir; G68 model yürütümü ayrıdır.

`eylema-retrieval`; UTF8 pencereleri, sürümlü temsil kimliğini, birim vektör iç çarpımını, sözcüksel küme Jaccard skorunu, deterministik hibrit skorları ve tam snapshot kalıcılığını yönetir. `eylema-host::retrieval` yalnız güncel yetkili ProjectStore içeriğini okur; kaynak başvurusu ve senkronize put özeti parça/aralık/metin özeti sonuçlarında korunur. Ham kütüphane değerleri yetki değildir. Üçüncü taraf crate veya veritabanı motoru eklenmedi.

## Desteklenen sınırlar

| Kabul | Tavan |
|---|---|
| Kaynak / kaynak baytı | 16 /her biri8192 UTF8 baytı |
| Parça / parça baytı | 128 /1024, skaler sınırında ve eksiksiz kapsam |
| Vektör boyutu | 1..4096, açık birim norm toleransı1e-5 |
| Sorgu baytı / benzersiz terim | 2048 /32, küçük harfli terim64bayt |
| Parça benzersiz terimi | 128 |
| İstenen sıra kesimi / dönen sonuç | 16 /32, kesimdeki tüm eşitlik grubu |
| Serileştirilmiş sonuç | Kaynak izi dahil65536bayt |
| Snapshot / kayıt / günlük | 1MiB /4 /5MiB |
| İşlem | 100M tanımlı iş birimi,64MiB mantıksal içerik,30s işbirlikçi süre |

Birleşik sınırlar en büyük bileşimi reddedebilir. Bilinmeyen token sınırı `TokenCounterUnavailable` döndürür; baytlar token diye sunulmaz. İç çarpım ve norm, saklanan F32 üzerinde sabit sırada F64 kullanır. Skorlar milyonda birliktir ve eşitlikte çift sayıya yuvarlanır; sözcüksel skor tam rasyonel küme Jaccard değeridir. Hibrit ağırlık açıkça1..999999 milyonda birliktir. Kaynak kimliği yalnız eşit skorların sunum sırasını belirler; tek kazanan yaratmaz. max_hits üstündeki eşitlik atomik hata verir.

Unicode davranışı Rust1.99 sürümüne bağlıdır: özgün en uzun alfanümerik diziler NFC, yerel ayar veya kök bulma iddiası olmadan küçük harfe çevrilir. Model/artifact/tokenizer/ön işleme parmak izleri ile anlamsal uzay/profil sürümleri tam eşleşmelidir. Değişen bağ yeni açık indeks gerektirir; yeniden etiketleme veya modele sessiz geçiş yoktur.

## Güncel yetki ve saklama

Oluşturma/değiştirme her kaynak ve atası için Collect ister; arama, indeks durumu gösterilmeden önce açık istenen her kaynak için Execute ister. Arama bütün sonucu hazırlar, ardından tüm kaynak/oturumları aynı kalıcı güvenilir zamanda denetler. Başka veya yeniden açılmış ProjectStore eski tutamakları geçersizleştirir. Açık yeniden açma güncel sahip/yeni Execute oturumu, eşleşen put özetleri ve tam güncel kaynak baytlarını ister.

Değiştirme tam sonraki nesli yazar. Kaynak değişimi yeni değişmez DataRef kullanır; dışarıda bırakılan kaynak güncel sonuçlardan silinir. Senkronizasyon veya yazı sonrası yetki/kontrol hatası onaylı nesil üretmez ve açık yeniden açma gerektirir. Ayrı veri/indeks günlükleri tek işlem değildir. Geçmiş diskte kalır: fiziksel silme, şifreleme, sıkıştırma veya düşmanca aynı kullanıcı yalıtımı iddiası yoktur. Bağdaştırıcı eğitim/dışa aktarma izni veya içerik paylaşma API'si vermez.

Katı ve Map içermeyen EYL ön denetimi, genel günlük ayrıştırıcısı bellek ayırmadan önce derinliği, düğümü, kapsayıcı uzunluğunu ve bayt/metin boyutlarını sınırlar. Bozuk özet, anlamsal snapshot, bilinmeyen sürüm, yarım son kayıt ve sırasız nesil eskiye dönmeden reddedilir. Mantıksal hesap elde tutulan/yeniden oynatılan/hazırlanan/serileştirilen kopyaları içerir; ayırıcı ek yükü/boşluğu, çağıranın G63 verisi, OS tamponu ve RSS ayrıdır. Yerel dosya çağrıları bloklanabilir;30s işbirlikçidir, kesin gerçek zaman değildir.

## Kanıt durumu

Sayısal kaynak58cec7c70bf8d536506bdcc90419adc518c6a27a üzerinde gerçek macOS dosya testi; oluşturma, senkronizasyon, işbirlikçi kilit, yeni yetkiyle kapatıp yeniden açma, üç skor türü, kaynak değiştirme/silme ve boş indeksi yeniden açmayı sentetik1024D vektörlerde geçti. Tam sıra `{a,d},b,c`; sabit ilgili kümeler için recall@2 ve recall@3=1.0. Bu deterministik fixture davranışıdır; model veya EN/TR ilgililik kalitesi değildir. Vektör/sözcüksel/hibrit yerel arama sayaçları4285333/4255047/4291121 tanımlı iş birimi ve83901/86721/86877 mantıksal bayttı; bunlar tek test gözlemidir, performans ölçütü değildir.

20erişim testi,9host erişim testi ve gerçek dosya kabulü geçti:128-eşitlik reddi, normalizasyonda kısalma/genişleme, doğru özetli14şema/sürüm/proje/temsil bağı/tekrar/sıra/aralık/terim/vektör/hash/nesil varyantı, yarım geçmiş, IO belirsizliği, durumdan önce izin reddi, ortak zaman süresi, hatalı vektör ön denetimi ve senkronizasyon sonrası iptal/süre sınırı kapsandı. Yerel test hedefi9ortak host testini tekrarlar; tekrarlar ek senaryo değildir. Özel ham iç çarpım testi2/1024/4096boyutta24literal skaler durumu<=2e-12farkla karşılaştırır; mikro/norm beklentileri değişmez. Tam yerel kontroller, güncel CI ve ayrı final-head incelemesi bağımsız teslim kapılarıdır; bu belge tek başına G70'i kabul etmez. Kaynak/oturum süresi ve IO hataları kontrollü enjeksiyondur; fiziksel güç kaybı veya gerçek kişi doğrulaması değildir. Ağırlık indirme, model/provider/eğitim yürütümü, GPU, SQL/graph, ANN, throughput veya genel erişim kalitesi sınanmadı.

Hedefli kontrolleri Rust1.99.0 ile çalıştır:

```sh
cargo +1.99.0 test -p eylema-retrieval --locked --offline
cargo +1.99.0 test -p eylema-host --locked --offline --test retrieval --test retrieval_native
```

Kanonik tam yerel Python/depo/gerçek-PR/diff kapıları, `tools/check_rust.py --provision`, güncel CI, gerçekten ayrı sahip yeterlilikli inceleme, yetkili merge ve merge sonrası main CI ayrı teslim kapılarıdır. Gerçek sahip yapılandırılmışgpt-6.1-sol/high profilini yalnız G70 sınırlı skaler/indeks/yetki kapsamı için kabul etti; gizli servis kimliği bilinmiyor. Hazırlık, onarım ve inceleme özgün120aktif dakika/3onarım/900inceleme saniyesinden düşer. Gerçek sahip yalnız iki ek hedefli onarım verdi (toplam5); sonuncu yalnız yeniden oynatma test kapsamını değiştirir, üretim baytları aynıdır ve aktif/inceleme süresi veya çağrı hakkı artmadı.
