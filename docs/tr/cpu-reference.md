# Sınırlı, kendi CPU primitive referansımız

İngilizce kanoniktir; [English](../en/cpu-reference.md). G23, bağımsız test edilen küçük primitive işlemler için kendi güvenli Rust F32 referans kitaplığımızı, `eylema-tensor` crate'ini sağlar. Tokenizer, model decoder, ağırlık yükleyici, tam Qwen çalışması, GPU/Wasm çalışması, eğitim veya hız/bellek performansı kanıtlamaz. Mevcut model destek durumları değişmez.

## Çağıran sözleşmesi

`Tensor::new` sonlu contiguous row-major değerlerin sahibi olur: rank1..4, pozitif checked boyutlar ve en fazla4096eleman. Yanlış rank, şekil/adet, checked overflow, NaN ve sonsuzluk kopyalamadan önce reddedilir. Değer/şekil public API'de değiştirilemez. Sonuç tam sahipli tensor veya hatadır; girdiler/eski cache hata durumunda değiştirilmez. Bellek rezervasyonları hata döndürebilir ve sınırlıdır. F16, BF16 ve quantized layout reddedilir; view, örtük broadcast ve dtype dönüşümü yoktur.

Referans aritmetiği indeks sırasıyla açık scalar F32 multiply/add, sonlu ara/çıktı kontrolleri kullanır; fused/SIMD/paralel yeniden sıralama yapmaz. Tüm işlem için matris işi en fazla262144multiply-add'dir; üç SwiGLU projection'ı birlikte sayılır. Bu sınırlı referans davranışıdır, optimize motor değildir. Standart Rust ve platform matematik/toolchain girdileri açıktır; Rust kullanımı host işletim sistemini kaldırmaz.

## Primitive arayüzler ve kanıt eşlemesi

İlk kimlik seti, kabul edilmiş özgün Qwen belirtiminin SHA-256 `acbbf68755dee5ca2b9d25bdcd3aa4206ce7156e032c38518a1fc66d7819fb06` hash'ine bağlı on altı `qwen3.*.v1` operatörüdür. [Kitaplık](../../crates/eylema-tensor/src/lib.rs) somut tip/fonksiyonları sunar; [referans vektörleri](../../crates/eylema-tensor/tests/reference.rs), [hata vektörleri](../../crates/eylema-tensor/tests/failures.rs) ve [kimlik sınırları](../../crates/eylema-tensor/tests/identity.rs) ayrı davranış kontrolleridir.

| Sürümlü operatör son eki | Kendi işlemimiz / bağımsız kanıt |
| --- | --- |
| `embedding_gather`, `linear_no_bias`, `tied_logits` | Geçerli tablo satırı, output-first `[out,in]` ağırlık ve tied tablo yönü; farklı tablo/token/matris fixture'ları |
| `rmsnorm_fp32`, `silu`, `swiglu`, `residual_add` | Son-eksen scale/epsilon, kararlı sigmoid dalları, gate/up çarpımı ve down projection, aynı-şekil residual; literal norm/aktivasyon/gated sonuç ve overflow reddi |
| `reshape_transpose`, `dtype_and_layout` | Checked eşit extent, materialized gerçek permutation, yalnız F32 layout; koordinatları farklı transpose ve unsupported precision |
| `rope_default`, `grouped_kv_repeat` | Açık position/theta/rotary prefix, split-half çiftler ve tam head oranı; sabitlenmiş frekans ve token/head sırası |
| `causal_mask`, `scaled_attention`, `softmax_fp32` | Açık position/geçerli key, tipli genişletilmiş maskeler, scale ve son-eksen normalization; gelecek/padding reddi, kaydırılmış skorlar, çoklu head, tam-maske/nonfinite reddi |
| `kv_cache_append`, `greedy_bounded` | Çağırana ait append kapasitesi ve head/token eksenleri; hata halinde eski cache korunur, skor sonlu ve eşitlikte en küçük özgün ID seçilir |

Embedding vocabulary<=64, width<=16 ve token adedi1..16. Linear rank2 `[rows,in]` ile `[out,in]` alır; bias yoktur. RMSNorm açık sonlu pozitif epsilon ve rank1 son-eksen scale kullanır. SwiGLU projection'lardan önce toplam birleşik işi denetler.

RoPE, GQA, attention ve cache için batch<=2, heads<=4, sequence<=16 ve head/value width<=16. RoPE position0..4095; theta sonlu pozitif, rotary prefix pozitif/çift ve width içindedir. Döndürülmeyen kanallar korunur. Parametreler açıktır: kaynak varsayılan theta1000000, RMS epsilon1e-6'dır; model adı varsayılanları kendiliğinden sağlamaz.

Causal mask `[batch,heads,Q,K]` olarak materialize edilir; position/geçerli-key uzunlukları ve tüm eksenler denetlenir. Tipli false bit key'i dışlar. Softmax izinli satır maksimumunu çıkarır; tamamen maskeli satır, sıfır tahmin etmek veya NaN üretmek yerine reddedilir. Attention uyumlu Q/K/V/mask eksenleri ve sonlu pozitif scale ister; GQA tekrarı açıkça talep edilmelidir.

Cache kapasitesi1..16. İlk append batch/head/key-width/value-width'i bağlar; sonraki append token lane'lerini her head için kapasite içinde birleştirir. `len`/`is_empty` ve immutable key/value getter'ları durum gösterir. Çağıran model/rol/adım kimliği değişince cache'i ayırmalı/bırakmalıdır; bu saf primitive durum aktarımına yetki veremez. Greedy aynı uzunlukta benzersiz özgün ID ile1..64sonlu skor alır. ID seçer; EOS/generation/sampling döngüsü değildir.

## Coverage'ın anlamı

`reference_coverage`, değişmez spec hash'i, tanınan benzersiz ID'ler ve en fazla16kayıt kontrol eder. Tam set **primitive kimlik coverage'ı** demektir; native model destek kararı değildir. Özgün modelin head sayısı16/width128 ve BF16 girdisi, kimlik coverage'ı tam olsa bile küçük profilin dışındadır; kimlik testi bunların reddini gösterir. Başka mimari, format veya eğitim operatörü kendi kabulünü ister.

Beklenen değerler elden literal veya bağımsız yüksek-hassasiyet referansıdır. Sonlu yayımlanan fixture'larda absolute hata<=3e-5 **veya**, sıfır olmayan referansta, relative hata<=3e-5 kullanılır. Sıfır referansta yalnız absolute hata kullanılır. Şekil, maske, hata ve ID sonuçları tamdır. Toleranslar fixture sınırıdır; evrensel doğruluk sözü değildir. Ayrı geçici kaynak kopyalarındaki aritmetik, transpose ve maske mutasyonlarını davranış fixture'ları yakalamalı; asıl kaynak korunmalıdır.

İnsan, özel üç-grup/13-test geliştirme denemesi ve ayrı Decimal80 incelemesi ardından yazılı tasarım, somut sohbet içi plan ve G23'e özel sayısal katkı profilini kabul etti. Bu yönetişim kararı tek başına üretimi kabul etmez. Mevcut izinle merge öncesinde tam yerel kontroller, güncel CI ve gerçekten ayrı incelenen sürüm kanıtı gerekir. Üçüncü taraf üretim/dev crate, gizli çıkarım/eğitim motoru, ağırlık/model/provider/eğitim denemesi veya izin değişikliği eklenmez.
