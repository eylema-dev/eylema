# Eylema — Öğrenme verisi ve eğitim protokolü — v0.1

Dil: Türkçe. Kaynak sürüm: 0.1.0. İngilizce kaynak belge: [Learning data and training protocol](../en/learning-and-training.md). Makinece okunabilir kimlik ve alan adları iki dilde aynıdır.

Tarih: 11 Ekim 2026. Durum: tasarım önerisi. Bu belge veri toplamayı, eğitim başlatmayı veya model dağıtmayı etkinleştirmez. Gerçek eğitim, dışa aktarma, performans ölçümü veya model kabul testi çalıştırılmadı.

## Amaç ve mimari sınır

Amaç; platform çalıştıkça yerel modelleri geliştirmeye yarayan, kaynağı bilinen ve doğrulanabilir örneklerin birikmesidir. Başarı daha çok kayıt veya daha düşük eğitim kaybıyla değil, aynı kaynak bütçesinde daha iyi tamamlanan görevlerle ölçülür. Büyük modeller ve insanlar öğretmen, inceleyen ve düzeltici olabilir; ürettikleri değerlendirmeler gözlenmiş sonuçlarla aynı kanıt sınıfına konmaz.

Üretimde kendi Rust bileşenlerimiz veri toplama politikasını, görev yürütmeyi, model atamasını ve kabul edilmiş model paketlerini yönetir. Kullanıcının açık seçimiyle Unsloth, eğitim ve paketleme ortamında kullanılan dış geliştirme aracıdır. Python, eğitim kütüphaneleri ve dönüştürücüler bu ortamın envanterine yazılır; Rust çalışma motorunun gizli bağımlılığına dönüşmez. Eğitim sunucusunun kesilmesi kabul edilmiş uygulamaların çalışmasını durdurmaz.

Akış: izinli olaylar → bölüm/geri bildirim kayıtları → seçme ve doğrulama → değişmez veri sürümü → sabitlenmiş eğitim tarifi → aday model → bağımsız değerlendirme → sürüm PR'ı → yetkili onay → sınırlı dağıtım → etkin sürüm veya geri dönüş. Beceri ve iş akışı öğretimi ağırlık eğitiminden ayrı sürümlenir; ölçülen ihtiyacı çözüyorsa model eğitimi zorunlu değildir.

## Kayıt sözleşmeleri ve izin

`Episode` bir görevin başlangıcından tamamlanmasına, iptaline veya zaman aşımına kadar yürütme bölümüdür. `episode_id`, görev/tenant kimliği, zamanlar, ortam sürümü, model ve beceri hash'leri, gözlem, önerilen eylem, uygulanan eylem, sonuç, hata ve kaynak ölçümlerini ilişkilendirir. Büyük görüntü/video içerikleri olayın içine gömülmez; izinli nesne referanslarıyla bağlanır. Kaydedilmeyen veya kaybolan olaylar açık boşluk işareti taşır.

`FeedbackRecord`; insan düzeltmesini, öğretmen önerisini, doğrulayıcı sonucunu ve itirazı kaynak kimliğiyle saklar. Eski değerlendirme sessizce değiştirilmez; yeni kayıt öncekinin yerine geçtiğini bildirir. Açıklama, araç çağrısı ve gözlenebilir sonuç yeterlidir; modellerin gizli düşünce zincirini toplamak gerekmez. `EvidenceRecord` çalıştırılan kontrolü, sürümünü, ortamını ve sonucunu gösterir. İzlenebilirlik, etiketin kesin doğru olduğu iddiası değildir.

İşletim telemetrisi izni eğitim izni sayılmaz. İçerik toplanmadan önce açık katılım seçimi ve `training_scope` belirlenir: `none`, `tenant_private`, `organization_shared`, `public_contribution`. Amaç, veri kategorileri, saklama süresi, izin kaynağı ve geri çekme yöntemi görünür olur. Dış öğretmen API'sine aktarım ayrıca kapsama bağlanır. Kullanıcının hizmetten yararlanması eğitim paylaşımını kabul etmesine bağlı tutulmaz.

Anahtarlar, kimlik doğrulama bilgileri ve gereksiz kişisel içerik toplama sınırında ayıklanır. Tenant ayrımı, erişim kontrolü ve saklama politikası ham nesnelerde de uygulanır. Değişmez sürüm kimliği, ham içeriğin sonsuza kadar tutulması anlamına gelmez: silinen veya izni geri çekilen içerik erişilemez kılınır, etkilenen paketlerin uygunluğu yeniden değerlendirilir. Eğitilmiş ağırlıklardan belirli bilginin otomatik silindiği varsayılmaz.

## Farklı amaçlar için farklı veri

| Veri paketi | Kaydedilen temel bilgi | Kullanım |
|---|---|---|
| Dil/araç kullanımı | Girdi, görünür bağlam, araç şeması, doğrulanmış çıktı veya düzeltme | SFT, görev yönergesi, araç seçimi |
| Tercih | Aynı girdiye iki aday, tercih, değerlendiren ve gerekçe | DPO gibi tercih eğitimi |
| Eylem yörüngesi | Sıralı gözlemler, uygulanan eylemler, ödül/maliyet, bitiş, iptal, eylem kısıtları | Davranış taklidi ve desteklenen politika eğitimi |
| Dünya geçişi | Önceki durum/gözlem, eylem, zaman aralığı, sonraki gözlem, ortam ve sensör sürümleri | Eyleme koşullu dinamik tahmini |
| Donanım performansı | Model hash'i, iş yükü, bağlam uzunluğu, eşzamanlılık, CPU/GPU, sürücü, bellek, gecikme | Model seçimi, atama ve kapasite profili |

Dil sohbetine çevrilmiş bir hareket kaydı, kendiliğinden uygun dünya modeli eğitim verisi olmaz. Kısmi gözlem, koordinatlar, birimler ve zaman adımı sözleşmenin parçasıdır. İptal ile doğal bitiş ayrılır; ödül sürümü tutulur. Sonradan öğrenilen sonuçlar karar anındaki girdiye sızdırılmaz. Donanım ölçümleri aynı iş yükü ve ortam tanımı olmadan karşılaştırılmaz.

Unsloth'un veri rehberi de ham metin, yönerge, sohbet ve tercih/geri bildirim biçimlerini ayırır; sentetik örneklerin kalite ve ilgililiğinin denetlenmesini ister. Bizim bölüm sözleşmemiz daha geniştir; eğitim adaptörü yalnız seçilen yöntemin beklediği biçimi üretir. [Unsloth veri rehberi](https://unsloth.ai/docs/get-started/fine-tuning-llms-guide/datasets-guide)

## Değerli örnek seçimi ve etiket güvenilirliği

Seçici; insan müdahalesi, başarısızlık, beklenmeyen sonuç, modellerin anlaşmazlığı, yeni görev kapsamı ve performans gerilemesini aday işaretleri olarak kullanır. Bunlar doğru etiket veya eğitim değeri garantisi değildir. Tekrarlanan kolay örnekler azaltılır; sorunlu örnekler, karşı örnekler ve kullanıcı düzeltmeleri inceleme kuyruğuna alınır. Yanlış eylem iyi davranış diye taklit ettirilmez; hata örneği veya düzeltilmiş yörünge olarak ayrı işlenir.

Yalnız zor vakaların seçilmesi gerçek kullanım dağılımını bozabileceğinden temsilî örnekleme kotası da bulunur. Görev türü, tenant kapsamı, dil, cihaz ve başarısızlık türü dağılımları raporlanır. Seçilme nedeni, örnekleme olasılığı uygulanabiliyorsa bu olasılık ve veri ağırlığı saklanır. Yalnız başarıların kalması engellenir; eylem seçiminin doğurduğu gözlem yanlılığı görünür tutulur.

`label_source` en az `observed_real`, `simulated`, `teacher_generated`, `human_reviewed` ayrımını taşır; bir örnekte birden fazla kaynak bulunabilir. Simülatör sürümü, öğretmen modeli/istem sürümü ve insan düzeltmesi ayrı referanslardır. Öğretmenin kendi cevabına yüksek puan vermesi kabul kanıtı değildir. Çalıştırılabilir doğrulayıcılar, bağımsız örnek denetimi ve değerlendirenler arası anlaşmazlık kuyruğu kullanılır. Üretilen tahmin gerçek sonraki durumun yerine geçirilmez.

## Değerlendirme ayrımı ve veri sürümleri

Temizleme ve tam/yakın kopya gruplaması, eğitim–geliştirme–test ayrımından önce yapılır. Aynı bölümün parçaları, aynı belgenin türevleri ve aynı sentetik kökün varyasyonları farklı bölümlere dağıtılmaz. Tenant dışına genelleme ve gelecekteki kullanım ayrı test hedefleridir; amaca göre tenant grupları ve ileri zaman pencereleri ayrılır. Her görev için körlemesine tek bir yüzde kullanmak yerine kapsam ve örnek sayısı gerekçelendirilir.

Nihai holdout; öğretmen üretimine, veri seçiciye ve hiperparametre aramasına açılmaz. Tekrarlanan denemelerde seçim yapılan küme geliştirme kümesidir; nihai test sonucu aday seçmek için kullanıldıysa sonraki bağımsız kabul için yeni, kilitli değerlendirme gerekir. Test örneğinin düzeltmesi eğitime alındığında ilgili değerlendirme sürümü emekliye ayrılır.

`DatasetRelease`; içerik manifesti/hash'leri, şema, kaynak hakları, kapsam izinleri, temizleme tarifi, kopya grupları, ayrım yöntemi ve etiketi kabul eden kanıtları taşır. Ham günlükten eğitim örneğine dönüşüm yeniden üretilebilir olur. Veri, eğitim, değerlendirme ve model sürümleri ayrı kimlik taşır; yeni sürüm eskisinin içeriğini değiştirmez. Silme veya erişim iptaliyle yeniden üretilemeyen paketler bu durumla işaretlenir.

## Unsloth eğitim adaptörü

Resmî rehber LoRA/QLoRA, tam ince ayar, ön eğitim, SFT, tercih optimizasyonu ve GRPO/GSPO örneklerini belgeliyor. Seçim görev, desteklenen mimari ve donanıma göre yapılır; ilk dar görevde SFT/LoRA veya QLoRA karşılaştırması yeterli olabilir. Eğitim kaybı tek başına kabul ölçütü değildir. [Unsloth eğitim rehberi](https://unsloth.ai/docs/get-started/fine-tuning-llms-guide)

Güncel ana depo dil modelleri yanında diffusion, decision, embedding ve ses modellerini de listeliyor. Bu geniş liste, kendi RSSM/Dreamer, MuZero veya başka özel ağımızın otomatik desteklendiğini kanıtlamaz. Her mimari için yükleyici, ileri hesaplama, kayıp/ödül, veri biçimi, eğitim yöntemi ve dışa aktarma yolu doğrulanır. Desteklenmeyen yol açıkça reddedilir; başka motor sessizce devreye alınmaz. [Unsloth resmî deposu](https://github.com/unslothai/unsloth)

`TrainingRecipe`; Unsloth commit/sürümü, bağımlılık kilidi, temel model hash'i/lisansı, tokenizer ve şablon, veri sürümü, eğitim yöntemi, hassasiyet, tohumlar, parametreler, ödül/doğrulayıcı sürümü, bütçe ve çıktı sözleşmesini sabitler. Çalıştırma `TrainingRun` kimliği üretir; planlanan, gerçekleşen ve erken durdurulan adımlar ayrılır. Aynı tarifin farklı donanımlarda bit düzeyinde aynı sonuç vereceği varsayılmaz.

## Dışa aktarma ve Rust kabulü

Unsloth GGUF kaydetme/nicemleme ve birleştirilmiş 16-bit çıktı yolları sunuyor; belgelenen manuel GGUF dönüşümü llama.cpp araçlarını kullanıyor. Kullanılırsa dönüştürücü sürümü eğitim/paketleme envanterine girer. GGUF veya safetensors dosyası olması, kendi Rust motorumuzun o mimariyi ve bütün operatörlerini desteklediği anlamına gelmez. [GGUF dışa aktarma](https://unsloth.ai/docs/basics/inference-and-deployment/saving-to-gguf)

Kabul sırası: eğitim referansı → beyan edilen hassasiyette nicemlenmemiş dışa aktarım → CPU referansı → kullanılacak her hassasiyet/nicemleme ve hedef backend. Nicemleme isteğe bağlıdır; kullanılmıyorsa uygulanamaz olarak kaydedilir. 16-bit çıktı otomatik olarak tam hassasiyetli referans sayılmaz. Tokenizer, sohbet şablonu, BOS/EOS, maskeleme ve ön/son işleme kontrol edilir; Unsloth belgeleri şablon ve özel token farklarının dış ortamda bozuk sonuç üretebildiğini belirtiyor. Numerik toleranslar önceden tanımlanır; görev başarısı, durma davranışı, çok adımlı tahmin ve kapalı döngü eylem sonuçları ayrıca ölçülür. [Çıkarım sorunları rehberi](https://unsloth.ai/docs/basics/inference-and-deployment/troubleshooting-inference.md)

## Sürümleme, ekosistem ve ilk teslim

Mevcut AI takım protokolündeki `NeedSpec`, `WorkPlan`, `ChangePackage` ve kanıt zinciri korunur. Yeni veri yeterliliği, doğrulanmış dağılım kayması veya performans gerilemesi eğitim ihtiyacı adayı oluşturur. Asgari yeni örnek/kapsam, bekleme süresi, eşzamanlı iş sınırı, GPU-saat bütçesi ve durdurma koşulları tarifte bulunur. Her günlük olay eğitim başlatmaz; aynı gerileme sınırsız yeniden deneme üretmez.

Otomasyon aday eğitimini, karşılaştırmayı ve sürüm PR'ını hazırlayabilir. PR; veri/eğitim/değerlendirme kimlikleri, model hash'leri, kod SHA'sı, donanım kanıtı ve önceki sürümle farkı içerir. Yetkili insan veya önceden onaylanmış politika ardından `ModelSetRelease` sınırlı dağıtıma çıkar. Eşik ihlalinde önceki uyumlu sürüm etkinleşir. Yeni ağırlık terfisi, onaylı küme içindeki çalışma anı atamasından ayrıdır.

Açık ekosistem; dataset, ödül, değerlendirme, model ve donanım profili paketleriyle katkı kabul eder. Paketlerde kaynak, katkıcı kredisi, lisans, izin kapsamı, şema sürümü, uyumluluk ve hash bulunur. Topluluk ödül kodu etkinleştirilmeden incelenir ve sınırlı ortamda çalışır. Geliştirme değerlendirmeleri paylaşılabilir; kilitli holdout örnekleri, özel tenant içeriği ve bunları geri çıkarabilecek ayrıntılar yayımlanmaz. Açık paketler otomatik güvenilir sayılmaz.

İlk teslim tek görevde izinli bölüm kaydı, insan düzeltmesi, temsilî veri seçimi, kilitli değerlendirme, bir Unsloth tarifi ve Rust kabul raporu olmalı. İzlenecek sonuçlar: uçtan uca başarı, hatalı eylem, insan müdahalesi, yalnız ayrıca izin verilmişse büyük modele devretme, p95 gecikme, bellek ve başarılı görev başına maliyet/ölçülebilen enerji. Yerel zorunlu görev buluta sessizce taşınmaz. Kayıt hacmi ve eğitim kaybı yardımcı tanıdır. Kazanç göstermeyen aday yayımlanmaz; neden ve öğrenilen bulgu saklanır.
