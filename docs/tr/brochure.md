# Neden Eylema — Broşür metni ve üretim kaydı

[English canonical](../en/brochure.md) · 11 Ekim 2026

[PDF](../../assets/eylema-brochure/why-eylema.tr.pdf) · [Düzenlenebilir metin](../../assets/eylema-brochure/copy.tr.json)

## Fikirden eyleme.

**Kontrol sende.**

Yapay zekâ ekiplerinin geliştirdiği, yerel ajanların işlettiği ve insanın yön verdiği web uygulamaları için bir platform vizyonu.

Net bir amaç. Yönü belli yapay zekâ.

Sürekli geliştirme ve işletim

ERKEN GELİŞTİRME  /  ÜRÜN VİZYONU

## Daha fazla olasılık. Daha açık kontrol.

Bir ihtiyacı çalışan yazılıma dönüştürmek; ortak bir hedef, açık yetkiler ve incelenebilir sonuçlar gerektirir. Eylema bunları bir araya getirmek için tasarlanıyor.

### Bir hedef. Ortak bir dil.

İnsan ve yapay zekâ için ortak bir sözleşme: uygulamanın ne yapacağını, hangi eylemlere izin verildiğini ve başarının ne olduğunu tanımla. Geliştirme ve işletimi aynı amaç etrafında buluştur.

### Yetkiyi sen belirle.

Hedefler, izinler ve bütçeler insanın yönetiminde. Tasarım; gerekli adımlarda onayı, görünür sonuçları ve müdahale olanağını içeriyor. Bir model kendi yetkisini genişletemez.

### Cihazındaki gücü işe kat.

Yerel ajanlar ve modeller, işe ve kullanılabilir kaynaklara göre planlanıyor. Amaç; kalite, gecikme ve bellek ölçümleriyle uygun yetenekleri seçmek. Geliştirme sağlayıcıları ve GitHub ise açıkça çevrimiçi.

### Gelişimi kanıtla.

Öğrenme tasarımı, izinli deneyim kayıtlarını ve insan düzeltmelerini değerlendirilen model adaylarına bağlıyor. Her iyileştirme, bağımsız değerlendirme görevleri ve kabul kontrolleriyle yerini hak etmeli.

## Geliştir. İşlet. Kanıtla. İyileştir.

Her adımında insanın yetkisi ve kabul ölçütleri tanımlı, birbirine bağlı bir döngü. Bu döngünün tamamı bir geliştirme hedefidir.

### HEDEFİ TANIMLA

İhtiyacı, izin verilen eylemleri, kaynak bütçesini ve beklenen sonucu tarif et.

### GELİŞTİR VE İNCELE

Yapay zekâ ekipleri değişiklikleri sürüm öncesi test, inceleme ve kabul için hazırlar.

### İŞLET VE GÖZLEMLE

Yerel ajanlar verilen yetki içinde çalışır. Sonuçlar ve müdahalenin etkisi görünür kalır.

### DEĞERLENDİR VE İYİLEŞTİR

İzinli kayıtlar geliştirmeye ve eğitime katkı sağlar. Adaylar kabulden önce yeniden değerlendirilir.

### Küçük çekirdek. Büyük olasılıklar.

Temelde projeye ait Rust bileşenleri var. 3B, yerel dil modelleri ve dünya modelleri isteğe bağlı yetenekler olarak planlanıyor. İşletim sistemi, tarayıcı, sürücüler ve eğitim araçları açık bağımlılıklar olarak kalıyor.

## Geleceği birlikte geliştirelim.

Eylema yolun başında. Net bir ihtiyaç, tekrarlanabilir bir deney ya da düşünülmüş bir katkı, projenin yönünü şekillendirebilir.

### Geliştir

Rust bileşenlerine, araçlara ve anlamlı testlere katkı ver.

### Araştır

Değerlendirme görevleri, donanım ölçümleri ve izinli öğrenme tarifleri geliştir.

### Anlaşılır kıl

Belgeleri, örnekleri ve İngilizce–Türkçe anlatımı iyileştir.

### BUGÜN NEREDEYİZ?

İlk Rust çalışma alanı, başlangıç komutları, doğrulanan görev tipleri ve iki dilli tasarım belgeleri mevcut. Platformun bütünü geliştirme aşamasında.

Görev oluşturma, yürütme, durdurma, kaydetme ve yeniden açmayı tarayıcıda göstermek. GPU, modeller, öğrenme ve otonom geliştirme için ayrı kabul adımları var.

**İlk katkınla yön ver.**

Projeyi incele. Bir ihtiyaç öner. Katkını şekillendir.

[github.com/eylema-dev/eylema](https://github.com/eylema-dev/eylema)

## Üretim ve kontrol

Dört A4 sayfa; mevcut Eylema logosu, turuncu, mürekkep ve kâğıt renkleri. PDF, dijital paylaşım ve normal A4 baskı için hazırlanmıştır; kesim payı veya CMYK baskı profili içermez. Markdown metni erişilebilir okuma ve düzenleme karşılaştırması içindir; PDF etiketli bir erişilebilirlik belgesi değildir.

Metnin kaynağı ilgili JSON dosyasıdır. Üretici PDF'yi ve bu Markdown metnini aynı girdiden oluşturur. İngilizce ve Türkçe anlamları gözden geçirildikten sonra çeviri parmak izleri yenilenir. Parmak izleri anlamsal eşdeğerlik kanıtı değildir.

```sh
python tools/render-brochure.py --font-dir <licensed-arial-directory> --verify
.venv/bin/python tools/check_repository.py --write-translation-hashes
```

Üretim araçları uygulama çalışma zamanına veya CI'ya eklenmez. Sürümler, kaynak ve çıktı özetleri [tooling.json](../../assets/eylema-brochure/tooling.json) dosyasında kayıtlıdır.

Araç mevcut kurulumları kullanır; ağdan indirme veya kurulum yapmaz. Yazı tipi dosyaları dağıtılmaz; PDF'ye yalnız kullanılan karakterlerin alt kümeleri gömülür. Lisanslı Arial Regular, Bold ve Black gerekir. Görsel kontrollerin çıktıları `.cache/brochure/` altındadır.

## İddiaların kaynakları

Mevcut durum için çekirdek deposunun aşağıdaki revizyonu esas alınmıştır. Daha eski tasarım belgelerindeki durum cümleleri bu ilerlemeyi geçersiz kılmaz. Geliştirme, yerel işletim, insan denetimi ve öğrenme döngüsü; tamamlanmış ürün özellikleri olarak sunulmaz.

- [Core status](https://github.com/eylema-dev/eylema/blob/7e64118196845814d9e8de531fb817c9f14fde5a/README.md)
- [Typed task contracts](https://github.com/eylema-dev/eylema/blob/7e64118196845814d9e8de531fb817c9f14fde5a/docs/en/typed-contracts.md)
- [Architecture](https://github.com/eylema-dev/eylema-docs-en/blob/4e8f91ce6515bb7d0a02aab89c97cfca8e086a4f/docs/architecture.md)
- [API and oversight](https://github.com/eylema-dev/eylema-docs-en/blob/4e8f91ce6515bb7d0a02aab89c97cfca8e086a4f/docs/api-reference.md)
- [Roadmap](https://github.com/eylema-dev/eylema-docs-en/blob/4e8f91ce6515bb7d0a02aab89c97cfca8e086a4f/docs/roadmap.md)

Bu broşür yeni çalışma zamanı, GPU, model, eğitim veya performans kanıtı üretmez. İnceleme bu Codex oturumunda öz incelemedir; bağımsız inceleme beklemektedir.
