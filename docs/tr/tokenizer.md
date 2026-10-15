# G29 sınırlı kendi tokenizer’ımız

İngilizce kanoniktir. [English](../en/tokenizer.md). İncelenmiş [tasarım](g29-tokenizer-design.md) profil sınırlarını sabitler. Kaynak PR, güncel CI ve gerçekten ayrı son-head inceleme teslim kaydına kadar bekler; yerel kabul merge onayı değildir.

## Davranış ve kimlik

`eylema_model::tokenizer::Tokenizer`, çağıranın yetkili reader’larını ve değişmez G27 manifest’ini kabul eder. Kurulmadan önce tokenizer/config’in bütün baytlarını doğrular; dosya/ağ açma, gizli engine, Python runtime veya denetimsiz public constructor yoktur. İlk üretim desteği yalnız Qwen3-0.6B manifest4629bayt/SHA256 `c087a033d4d193acd9912d0f555c99f14f9b968c9f308aca8acb52b07af85af2`, kaynak `c1899de289a04d12100db370d81485cdf75e47ca` içindir. Başka manifest/şablon/context/precision değişimi kendi kabul edilen profilini bekleyerek açıkça reddedilir. Metin ön ucu `check_assignment` sonucunu `UnsupportedLocal` ötesine taşımaz.

Tam11422654bayt tokenizer SHA256 `aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4`; config9732bayt SHA256 `d5d09f07b48c3086c508b30d1c9114bd1189145b74e982a265350c923acd8101`; şablon4168bayt SHA256 `a55ee1b1660128b7098723e0abcd92caa0788061051c62d51cbe87d9cf1974d8`. Açık kaynak verisi yayıncı koşullarını korur; model ağırlığı dağıtılmaz. Onaylı20MiB içinde yalnız açık tokenizer/Unicode bildirim verisi okundu; kimliği doğrulanmış önbellek config’i kullanıldı.

Beş kendi Rust birimimiz sınırlı kabul, Unicode16 NFC, sabit sıralı bölme, tersinir byte-BPE ve tipli prompt/strict decoding sağlar. Fallible heap-sorted indeksler gizli allocation kullanmaz; kriptografik olmayan sözlük lookup ipuçları tam string eşitliğini her zaman denetler. Kriptografik kimlik sayılmazlar. Kimlik hash’i kabul edilmiş macOS CommonCrypto akışıdır; yazdığımız kriptografi veya üçüncü taraf runtime crate’i yoktur.

## Metin ve tipli prompt alt kümesi

`encode_text`, `encode_prompt`, `decode` açık çağıran `TextBudget` alır; ID ve kaynak digest erişimi readonly’dır. `PromptMessage` isteğe bağlı ilk System, ardından dönüşümlü User/Assistant ve son User, en fazla8mesaj destekler. Sabit no-tools/non-thinking önek yeniden üretilir; rastgele role/Jinja/tool/multimodal/reasoning-field engine yoktur.26added-token literal yazımının tamamı düz girdide normalizasyondan önce ve sonra reddedilir. Yapısal ID’yi yalnız tipli formatter ekler. Added ID literal veri olarak decode edilir; görev/eylem yetkisi değildir.

Unicode16 unassigned scalar, bozuk/eksik UTF8 decode, bilinmeyen ID, duplicate alan/ID, eksik byte/merge referansı, desteklenmeyen semantik, değişmiş/kesik/fazla artifact ve tükenen sınır kategorik ret olur. Debug sayı/digest veya role/içerik bayt sayısı gösterir; ham metin, özel metaveri, I/O mesajı ve yol göstermez. Persistence/collect/train/ekosistem/proje export yeteneği yoktur.

## Kaynak ve platform sınırları

| Sınır | İlk profil |
|---|---|
| Artifact girdisi | Tokenizer16MiB/config64KiB; <=160000vocab ve160000merge; decode edilmiş kayıt4096bayt |
| İstenen resident içerik | Tutulan veri, ham/parser/indeks/scratch/çıktı ve Unicode çalışma görünümü dahil<=128MiB; fallible reservation büyümeden önce denetlenir |
| Okuma | EOF dahil ortak1024çağrı/16Interrupted tekrar; bilinen boyut/profil/resident sınırı girdiden önce reddeder |
| Kabul işi | Artifact başına<=134217728birim; değişmez parse başına1000000mantıksal node/depth16; tekrar görünüm ortak işi sayar |
| Metin/prompt | Metin başına8192UTF8bayt;8mesaj/32768toplam içerik; normalizasyon toplam çalışma scratch64KiB |
| Çıktı/context |4096token üst sınırı; prompt context4096’dan generation128’i checked subtraction ile ayırır |
| Metin işi/decode | <=2000000scan/normalization/merge birimi; strict decode<=4096ID/65536çıktı baytı |

Bunlar istenen içerik/iş sınırlarıdır; ölçülmüş RSS, throughput veya model kapasitesi değildir. Rastgele bloklayan reader deadline’ı çağırana aittir. macOS native kabulü seçilmiş mevcut portable-library Wasm derlemesinden ayrıdır; başka hedefte tokenizer yürütmesi kabul edilmez.

## Veri kökeni ve gerçek kanıt

Üretilen Unicode verisi287516bayt/SHA256 `6797aff88e12c5a56e4e28a7be08b66dfa7e2bcd9d37e6c834b7f7c407100fe6`; mevcut incelenmiş CPython3.14/Unicode16 verisinden yeniden üretildi. [Unicode bildirimi](../../crates/eylema-model/data/UNICODE-LICENSE.txt) ve [köken kaydı](../../crates/eylema-model/data/unicode16.provenance.json) taşır. Property/composition kaynak kimlikleri `3627a624f63370f0e726dde726f42934c8ca898c5e18fb69abdeee5a27ff00d8` ve `e83834fdad296e46779fb702e11dd9b8686409a6346461d02fe3a22afaf26dd4`. Python yalnız geliştirmededir; runtime normalizasyon/sınıflandırma kendi Rust’ımız ve değişmez atıflı veridir.

Bağımsız dondurulmuş referans SHA256 `01c8a64e4142175690a15282b6913f53508e9b9e9d66cd0ac8c84c7b6c577ff5` üretim kodundan öncedir. Dört stdlib anchor geçti; yayımlanmış Hello-world ID [9707,1879]. Gerçek tam artifact native kabulü13yazılmış EN/TR/Unicode/space/CRLF/overlap ve4tipli prompt için tam ID/strict decode karşılaştırır. Tokenizer kanıtıdır; Türkçe model kalitesi veya neural inference değildir. Normal CI iki özel tutulan açık-asset kabul testini dürüstçe ignore eder; açık yerel artifact girdisi gerekir. Küçük sentetik veri üretimde bulunmayan test-only sabit beklentiyle aynı kabul edilmiş algoritmayı çalıştırır.

Üç sınırlı onarım sayıldı: sınırı artırmadan tekrar sözlük işini azaltma/bütçe hata kategorisini koruma; beklenen ID’yi değiştirmeden dondurulmuş referansın Rust test aktarımında CRLF baytını koruma; aralıklı surrogate escape’i reddetme ve toplam normalizasyon scratch’ini büyümeden önce sınırlama. Son teslim tam kontroller ve derlenmiş kaynak mutasyonu kayıtlarını gerektirir. Ağırlık, referans engine, model/provider/eğitim çağrısı, GPU veya performance kabulü iddia edilmez.

Son yerel tüm-workspace kontrolü:350native geçti/3açık ignore (mevcut Metal ve iki özel tutulan açık-asset örneği);47Python geçti; tam fmt/clippy/build/bağımlılık/seçilmiş mevcut Wasm derlemesi geçti. İki açık yerel asset örneği geçti; dört derlenmiş kaynak mutantı başarısız oldu, geri alınmış özgün kaynak geçti. CI ignore örneği yürütülmüş sayılmaz. Ayrı son-head inceleme/güncelCI/koordineli main teslimi bekler.

Bilinen girdi baytı işlem sınırı ve iki EOF kontrolünü içeren ortak asgari okuma sayısı, ham bellek ayırmadan veya iki okuyucudan herhangi birini çağırmadan önce denetlenir. Bu gerekli alt sınırları geçmek; ayrıştırma işinin, kısa okumaların veya yeniden denemelerin bütçeye sığacağını garanti etmez; artımlı denetimler korunur.
