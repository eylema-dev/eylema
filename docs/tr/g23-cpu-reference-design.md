# G23 sınırlı CPU referansı — inceleme için yazılı tasarım

Tarih:14Ekim2026. Durum:tasarım önerisi; tensor crate, sayısal yeterlilik veya operatör kabulü yoktur. [English canonical](../en/g23-cpu-reference-design.md).

## Amaç ve yetki

Kullanıcı, kabul edilmiş G80sonrasında dar G23değerlendirme/kapsam hazırlığını onayladı. Yerel dil/karar/algı/dünya/policy profillerine temel olacak kendi Rust CPU referans operatörlerimizi kuracağız. Her mimari bağımsız doğrulanmış operatör/profil kapısını korur; tek Qwen-benzeri referans evrensel destek sağlamaz. Kapsam hazırlığı onayı, bu yeni yazılı tasarımı, sonraki uygulama planını, sayısal yeterliliği, tam modelleri veya performansı kabul etmez.

Core temel`177e9d322b82682683ae84ec8e84a13a04de11e9`; kabul edilmiş guide temel`2d76bf240da23682db2fd0f7f9e59ec89f52c5b5`. Aktivasyondan önce güncel kabul edilmiş G18/G27kanıtı ve main yeniden bağlanır. Mevcut platform CPU toplaması dar Metal kanıtı için test-only integer referansıdır; `eylema-tensor` crate'i yoktur. Mevcut sözleşme/model manifest'i ve destek durumları korunur.

## Yaklaşım

| Yaklaşım | Karar |
| --- | --- |
| Ayrı kendi saf CPU referans crate'i | Seçilen: sayısal/şekil davranışı bağımsız test edilir; host kabulü ve model decoder ayrı kalır. |
| Kernel'i mevcut platform Metal kanıtına koyma | Dar cihaz kanıtı korunur; farklı anlam ve ömürler profil kabulünü bulanıklaştırır. |
| Başka tensor/inference motoru aktarma | Mevcut first-party üretim politikasına aykırıdır; tasarım böyle bağımlılık eklemez. |

`crates/eylema-tensor`, güvenli kendi Rust kodumuzdur (`forbid(unsafe_code)`); filesystem/ağ/süreç/erişim bilgisi/donanım/provider erişimi olmayan saf hesaplama kütüphanesidir. Root `Cargo.toml`/`Cargo.lock` içine yalnız bu first-party crate kaydedilir. r4kapsam hazırlığı kayıt yollarını kapsar; üçüncü taraf üretim/dev crate veya gizli motor yoktur. Mevcut görev/EYL/journal/sürüm/kabul/host arayüzleri ve yeterlilik kayıtları korunur.

## Tipli sınırlar ve sayısal sözleşme

Kendi `Tensor` tipimiz sonlu F32 değerleri ve kanonik bitişik row-major shape taşır; rank1..4, pozitif boyut ve checked extent en fazla4096elemandır. Constructor hesaplamadan önce shape/extent uyuşmazlığı, sıfır boyut, taşma, NaN ve infinity'yi reddeder. Stride/view/örtük broadcast/quantization/BF16/FP16 düzen desteği yoktur. Her operatör tüm girdileri ve sonlu ara/çıktı değerlerini doğrular; hata kısmi sonuç döndürmez. Ayırılan sonuç tamponu aynı extent sınırına tabidir; crate gerçek cihaz belleği rezerve etmez veya kabul davranışını değiştirmez.

Referans hesabı, fused/SIMD/paralel yeniden ilişkilendirme olmadan deterministik indeks sırasıyla açık F32 işlemleridir. Matrix multiply-add sayısı checked ve işlem başına en fazla262144olur. Dot/normalization/softmax aynı sıralı F32sözleşmesini kullanır; bağımsız beklentiler üretim helper'ı değil elle sabit değer ve development-only Decimal80 veya ayrı yazılmış yüksek hassasiyet hesabıdır. Eşikler uygulamadan önce sabitlenir: sonlu yayımlanan fixture'larda mutlak hata<=3e-5 veya göreli hata<=3e-5; shape/index/mask/ID/hata sonuçları tam eşittir. Bunlar test toleransıdır; evrensel sayısal garanti veya performans iddiası değildir.

## İlk açık operatör profili

Mevcut Qwen sözleşmesindeki on altı sürümlü operatör ID'si ilk sonlu referans kapsama setidir; değişmez specification hash'lerini kabul edilmiş profil sağlar. Tam public tip/fonksiyon ve fixture kodlaması insan tarafından incelenen uygulama planında sabitlenir; host adapter uydurmaz.

| Operatör grubu | Gereken davranış ve sınırlar |
| --- | --- |
| embedding gather / tied logits / bias'sız linear | Geçerli token satırı, beyan yönü, uyumlu boyut; embedding vocabulary<=64,width<=16; linear rank2ve açık matrix boyutu; logits gizli kopya olmadan beyan tied table'ı kullanır. |
| RMSNorm / SiLU / SwiGLU / residual add | Açık epsilon>0; son eksen normalizasyonu, elementwise uyumlu shape ve sonlu sonuç; SwiGLU sabitlenmiş gated-product tanımını izler. |
| reshape/transpose / dtype-layout | Checked eşit extent ve gerçek eksen permütasyonu; materialized row-major çıktı; yalnız F32düzeni desteklenir. |
| default RoPE | Açık positions0..4095, sonlu theta>0ve çift rotary boyutu; batch<=2,headcount<=4,sequence<=16,headwidth<=16; model adından context/window/theta varsayılmaz. |
| grouped-KV repeat / causal mask / scaled attention / F32 softmax | Tam query/KV head oranı, açık mask/position ve pozitif scale; aynı sonlu eksen sınırları. Softmax satır maksimumunu çıkarır. Masked girişler sonlu olmayan tensor değil tipli mask bitidir; tümü maskeli satır reddedilir, NaN veya tahmini sıfıra dönüşmez. |
| KV cache append / bounded greedy | Cache çağırana aittir; açık güncel length/capacity<=16ve shape binding; bozuk append replacement döndürmez. Greedy sonlu beyan skorları ve eşit maksimumda en küçük özgün token ID'yi kullanır; generation döngüsü veya sampling yoktur. |

Her grup kabul edilmiş kaynak operatör anlamına bağlanır. Desteklenmeyen operatör/düzen/profil açık kalır; model biçimi veya başka mimari adı/rolü eşleşti diye kabul edilmez. Dil/reranker/embedding manifest'leri uygun primitive'i paylaşabilir; profil bütünlüğü/desteği sonraki loader/tokenizer/decoder/native çalışma kapılarındadır. Mevcut Qwen profilleri tam native çalışma için desteklenmeyen kalır.

## Bağımsız kabul ve gözetimli yeterlilik

Asıl G23-P1/A1/CP1korunur. Fixture'lar seçilmiş her operatörü simetrik olmayan matrix, farklı eksen/token satırı, shifted softmax, normalization, gated product, positional pair, GQA/causal/padding mask, append reddi ve deterministik tie ile kapsar. Negatifler bozuk shape/unit/axis/index/ratio/epsilon/theta/rank, extent/work taşması, NaN/infinity/sonlu olmayan ara değer, full mask ve desteklenmeyen kimlik/düzendir. Bağımsız profil testleri tam primitive kapsamını eksik operatörden ayırır; end-to-end model veya gizli fallback çalışmaz. Bilinçli aritmetik/index/mask bozukluğu, kaynak metni assertion'ı değil fixture tarafından yakalanmalıdır.

Mevcut kullanıcı onaylı `sol61-high-contracts-2026-10-14`profili tasarım/sözleşme/dev-test/belgeyi kapsar ve sayısal kernel'i açıkça hariç tutar. Kullanıcı, aynı yapılandırılmış `gpt-6.1-sol/high`ve bilinmeyen servingidentity ile dar sayısal değerlendirme hazırlığını onayladı. Tasarım açık gözetimli trial-only CPU referans katkı kapsamını önerir: bağımsız vektör, sonlu negatif regresyon ve gerçek ayrı yeterli inceleme kanıt oluşturur; PRyeterliliği/merge öncesinde değerlendirilen CPU referans kapsamını insan gerçekten kabul etmelidir. Kayıt veya bu tasarım sayısal üretim çalışmasını kendi başına yetkilendirmez. Gerçek karar/uygunluk kod aktivasyonundan önce sabitlenir; eksik yeterlilik pending kalır.

Değişmemiş tam native/repository/PR/fingerprint/whitespace kapıları tam revizyonla çalıştırılır; başlangıç riski/hata kayıtları korunur, skipped/not_run doğru belirtilir. Operatör doğruluğu ağırlık/tokenizer/tam model/Metal/Wasm çalışma/eğitim/throughput veya deneysel bellek performansını kanıtlamaz. Seçilen portable compilation açık owned-package envanteri ister; derleme hedef çalışması değildir.

## Teslim ve kaynaklar

Önerilen yollar: `crates/eylema-tensor/`, root `Cargo.toml`/`Cargo.lock`kaydı, eş EN/TR G23not/tasarımı ve çeviri fingerprint; gereken yalnızG23guide/kart/kaynak pin/plan/tarihli değerlendirme kayıtları. Model-contract/schema/checker/izin/G40değişikliği yoktur. Asıl120aktif dakika/3hedefli onarım/15ayrı inceleme dakikası, sıfır provider/model/ağırlık/eğitim denemesi ve yeni harcama; yeterlilik hazırlığı bütçeyi sıfırlamaz. Tek kapsamlı dal, özel kullanım defteri yenilemesi, ayrı tam commit incelemesi/güncelCI/mevcut izinle merge ve eş main tree/mainCIkanıtı gerekir.

Bu yazılı tasarım insan incelemesini bekler. Onaydan sonra mevcut sohbet içi yöntemle somut plan hazırlanır; gerçek sayısal katkı kabulü ayrı kalır ve aktivasyondan önce tamamlanmalıdır.
