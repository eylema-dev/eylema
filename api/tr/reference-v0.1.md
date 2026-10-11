---
docs_language: tr
source_revision: 0.1.0
status: design
canonical_language: en
---

# Eylema — Ortak API ve insan denetimi — v0.1

[İngilizce belge](../en/reference-v0.1.md) esas kaynaktır; bu belge onun tam Türkçe çevirisidir. İşlem kimlikleri, alan adları, durumlar ve hata kodları her dilde İngilizce kalır. Belge, önerilen bir sözleşmeyi tanımlar. Adlandırılan uç noktalar, SDK, üreticiler ve uygunluk testleri bu belgeyle uygulanmış veya çalıştığı gösterilmiş değildir. Bu sürümde OpenAPI tanım dosyası teslim edilmez.

## Tek sözleşme, farklı istemciler

İnsan arayüzleri, geliştirme ajanları, yerel modeller ve deterministik Rust becerileri aynı işlemleri ve yetkilendirme kurallarını kullanır. Sağlayıcı kimliği, model yeteneği veya bir insanın adı kendi başına izin oluşturmaz. Denetleyici doğrulanmış özneyi, kaynak kapsamını, kabul edilmiş politika revizyonunu ve gereken onayı belirler. `authorization_ref` gibi bir metin kaydın bulunmasını sağlar; yetki kanıtı değildir.

Kendi geliştirdiğimiz sürümlü tip tanımları, makine sözleşmesinin esas kaynağı olacaktır. Önerilen üretim süreci bu tanımlardan OpenAPI, JSON Schema, MCP araç açıklamaları ve Rust SDK üretir. Adaptörler yetkiyi genişletemez, varsayılan değer uyduramaz veya başarısızlığı başarıya çeviremez. Bu üretim tasarımı üçüncü taraf üretim bağımlılığı getirmez. Mevcut şemalar seçili teslim nesnelerini kapsar; bu API'nin tamamını tanımlamaz.

İzinler işlem bazında birleşir: modeli incelemek onu eğitme; sürüm önermek onu etkinleştirme yetkisi vermez. Bir kurulum aynı özneye birden fazla izin tanımlarken belirli geçişlerde ayrı onay isteyebilir. Tarayıcı istemcisi isteğine `native_linux` yazarak yerel dosya sistemi, süreç veya GPU ayrıcalıkları kazanamaz.

## Denetim mekanizmaları

Avrupa Komisyonu HITL, HOTL ve HIC'yi denetim yaklaşımları olarak açıklar. Bunlar **birlikte uygulanabilen mekanizmalardır**; birbirini dışlayan modlar veya kullanıcı rolleri değildir. HOTL sistem tasarımı sırasında gözetimi de kapsar; çalışma sırasındaki izleme kavramın tamamı değildir. HIC, sistemin kullanılıp kullanılmayacağına ve nasıl kullanılacağına karar vermeyi içerir. [Komisyon özeti](https://digital-strategy.ec.europa.eu/en/library/ethics-guidelines-trustworthy-ai), [13 numaralı dipnottaki açıklamalar](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:52019DC0168).

Aşağıdaki uygulama karşılıkları bizim tasarım tercihimizdir; mevzuata uyum iddiası oluşturmaz:

- **Human-in-the-loop (HITL):** belirlenmiş karar noktalarında insan katılımıdır. Politika, sonuç doğuran bir eylemden, veri kümesi kabulünden veya sürüm geçişinden önce insan kararı isteyebilir. Her rutin eylemi tek tek onaylatması gerekmez.
- **Human-on-the-loop (HOTL):** kanıtı ve davranışı inceleme, olayları izleme ve yetkili komutlarla müdahale etmedir. Tasarım incelemeleri ve işletim gözetimi birlikte katkı verir. Durdurma isteği, kesintinin gerçekten etkili olup olmadığını bildirmelidir.
- **Human-in-command (HIC):** amaçları, bütçeleri, yetki devrini ve sürümlü politikaları belirleme; izin verilen kararları geçersiz kılma, devri geri çekme veya kullanımı sonlandırmadır. HIC tekil eylemleri onaylamaktan daha geniştir.

`oversight_requirements`, ayrı ayrı uygulanabilen karar noktalarını, gözlem/müdahale gereklerini ve yönetim referanslarını kaydeder. Yalnızca `HITL`, `HOTL` veya `HIC` içeren tek seçenekli bir enum değildir. Modeller kendi devraldıkları yetkiyi genişletemez. Politika değişiklikleri önerdikleri yeni politikaya göre değil, halen kabul edilmiş politikaya göre değerlendirilir.

## Ortak işlem metaverisi

Yayınlanacak her işlem, uygulaması kabul edilmeden önce şu şablonu sağlamalıdır:

| Metaveri | Zorunlu anlam |
|---|---|
| `operation_id`, `operation_version` | Kalıcı kimlik ve uyumlu sözleşme sürümü |
| `request_schema_ref`, `response_schema_ref`, `error_schema_ref` | Tam sürümler, zorunlu alanlar, sınırlar ve reddedilen örnekler |
| `side_effect_class`, `resource_scope` | Okuma, kayıt değişikliği, hesaplama veya dış etki; tam yetkili kaynak kapsamı |
| `execution_context`, `required_capabilities` | Desteklenen `browser` / `native_linux` hedefleri ve gerçek önkoşullar |
| `permissions`, `oversight_requirements` | Özne izinleri ve birlikte uygulanan denetim yükümlülükleri |
| `preconditions`, `expected_revision` | Durum varsayımları ve eşzamanlılık kontrolleri |
| `approval_binding`, `policy_revision` | Geçerli karar kaydı ve belirleyici politika |
| `deadline`, `budget`, `max_concurrency` | Zaman, kaynak ve eşzamanlılık sınırları |
| `idempotency`, `retry`, `cancellation`, `compensation` | Tekrar, kurtarma, kesinti ve telafi kuralları |
| `audit_events`, `result_evidence` | Gözlenebilir geçişler ve başarı için gereken kanıt |

Yetenekler kullanılabilirliği anlatır; izin vermez. Kullanılamayan GPU, açık bir desteklenmiyor sonucu veya önceden yetkilendirilmiş alternatif çalışma yolu doğurur. Bilinmeyen bellek kapasitesi bilinmiyor olarak kalır. Her işlem sınırlarını yayımlar; istemciler bunları model açıklamasından varsaymaz.

## Taslak işlem kataloğu

Yollar, önerilen kontrol servisi rotalarıdır; canlı URL değildir. Her satır ortak metaveri şablonunu devralır. Bağlam kontrol servisidir; `execution_context`, yalnızca iş gönderilen işlemlerde desteklenen çalışan hedefini seçer. Okuma işlemleri alan durumunu değiştirmez; erişim kaydı tutulabilir. Katalog bir başlangıç kapsamıdır, bütün CRUD işlemlerini kapsamaz.

| `operation_id` | Yöntem ve yol | Etki ve kaynak kapsamı |
|---|---|---|
| `inspectRuntime` | `GET /v0.1/runtime` | Yetki kapsamındaki çalışma durumunu ve gözlenen sınırları okur |
| `listCapabilities` | `GET /v0.1/capabilities` | İşlem, hedef ve backend kullanılabilirliğini okur |
| `createTask` | `POST /v0.1/tasks` | Görev amacı, kapsamı ve bütçesini kaydeder |
| `getTask` | `GET /v0.1/tasks/{task_id}` | Görev görünümünü ve ilişkili kayıtları okur |
| `createActionProposal` | `POST /v0.1/action-proposals` | Değişmez plan revizyonunu ve eylem içeriğini kaydeder |
| `getActionProposal` | `GET /v0.1/action-proposals/{proposal_id}` | Önerilen etkileri, önkoşulları ve inceleme kanıtını okur |
| `createApproval` | `POST /v0.1/approvals` | Yetkili onaylayanın kapsamı belirli kararını kaydeder |
| `revokeApproval` | `POST /v0.1/approvals/{approval_id}/revocations` | Gelecekte kullanımı geri çeker; karar geçmişini korur |
| `createExecution` | `POST /v0.1/executions` | Tam olarak yetkilendirilmiş öneriyi yürütmeye gönderir |
| `getExecution` | `GET /v0.1/executions/{execution_id}` | Yaşam döngüsünü, etkileri ve sonuç kanıtını okur |
| `requestExecutionCancellation` | `POST /v0.1/executions/{execution_id}/cancellation-requests` | İlgili yürütmenin kesilmesini ister |
| `listEvents` | `GET /v0.1/events` | Kapsamlı ve cursor ile sayfalanan denetim olaylarını okur |
| `registerModel` | `POST /v0.1/models` | Model revizyonunu, dosyalarını ve uyumluluk iddialarını kaydeder |
| `registerBehaviorContract` | `POST /v0.1/behavior-contracts` | Dünya/politika/beceri anlamlarını ve durum kurallarını kaydeder |
| `createDatasetVersion` | `POST /v0.1/datasets/{dataset_id}/versions` | Değişmez veri kökenini ve kabul kararını kaydeder |
| `createTrainingRun` | `POST /v0.1/training-runs` | Yetkili veri sürümünde bütçesi sınırlı eğitim planlar |
| `createEvaluationRun` | `POST /v0.1/evaluation-runs` | Sabit ölçüt ve donanımla değerlendirme planlar |
| `createRelease` | `POST /v0.1/releases` | Değişmez aday model takımı manifestini kaydeder |
| `transitionRelease` | `POST /v0.1/releases/{release_id}/transitions` | Yönetilen onay, dağıtım, geri dönüş veya kullanım sonlandırma ister |
| `createPolicyRevision` | `POST /v0.1/policies/{policy_id}/revisions` | Yönetim, yetki devri veya geri çekme revizyonu ister |

Kayıt, rastgele model dosyalarını indirmez, çalıştırmaz veya güvenilir kabul etmez. Referanslar yalnızca yetkili kaynaklar içinde çözümlenir. Kaynak oluşturma, kimliği ve revizyonu döndürür. Asenkron iş, görev/yürütme referansı ve kabul durumunu döndürür; kabul tamamlanma değildir. Önerilen HTTP anlamları oluşturulan kayıtlar için `201`, kuyruğa alınan işler için `202` kullanır; hatalar makinece okunabilir gerekçe taşır.

Eğitim, değerlendirme, politika etkinleştirme ve sürüm geçişi, beyan edilen etkileri gerektirdiğinde öneri/onay/yürütme yaşam döngüsünü kullanır. Politika adayı oluşturmak politikayı etkinleştirmez. Plan adımlarının kendi etki kapsamları bulunur; üst amacı onaylamak yeni etkilere sessizce izin vermez. Yönetim değişiklikleri halen zorunlu olan karar noktasını atlayamaz.

## Onay ve yürütme

Onay; `proposal_id`, `plan_revision`, standartlaştırılmış `payload_sha256`, `state_revision`, `policy_revision`, `resource_scope` ve `expires_at` alanlarına bağlanır. Birlikte çalışabilir onay uygulanmadan önce standart serileştirme ve hash kuralları sabitlenmelidir. Karar kaydı doğrulanmış özneyi belirtir; doğrulanmış insan onayı gerektiğinde kendiliğinden yazılmış `human` alanı yeterli olmaz.

Yürütmede denetleyici izin, geri çekilme, süre aşımı, bütçe, içerik ve mevcut önkoşulları yeniden kontrol eder. Değişmiş plan veya önemli durum değişikliği bağlantıyı bozduğunda yeni karar gerekir. Desteklenen kaynak adaptörleri beklenen revizyona koşullu yazma yapar. Böyle garantileri olmayan dış dünyada kalan yarış koşulu ve gözlenen sonuç açıklanır; atomiklik iddia edilmez.

Geri çekme, etkili olduktan sonra gelecekteki yetkili kullanımı engeller; gerçekleşmiş eylemi geçmişten silmez. Geçersiz kılma komutları kendi izinlerini ve denetim kayıtlarını gerektirir. Yürütme her etkiyi ve o etkinin doğrulanmasını ayrı kaydeder.

## Olaylar, zamanlama ve hatalar

İstekler `request_id`, sözleşme sürümü ve ilişkilendirme bağlamı taşır; mevcut kaynağı değiştiren istekler gerektiğinde `expected_revision` içerir. Olaylar `event_id`, `task_id`, ilgiliyse `execution_id`, durum revizyonu, zaman damgası ve kanıt referansları taşır. İz özetleri gerekçeleri, araç girdilerini ve gözlenen sonuçları kaydeder; modelin gizli düşünce zincirini kaydetmez.

`listEvents`, kapsam filtreleri, sınırlı `limit` ve içeriği yorumlanmayan `cursor` alır; yanıt `next_cursor` içerir. Sıralama yalnızca tanımlı akış içinde garanti edilir; bütün sistem için garanti edilmez. Aynı olay tekrar teslim edilebilir; `event_id` ile tekrarlar ayıklanır. Cursor süresi dolunca `CURSOR_EXPIRED` döner; istemci güncel görev/yürütme görünümünü alıp yeni cursor ile devam eder. Saklama süresi ve görünür içerik gizlemeleri kurulum bazında belgelenir.

Hatalar `code`, insanın okuyabildiği `message`, isteğe bağlı `field_path`, `retryable`, `correlation_id` ve isteğe bağlı `retry_after_ms` içerir. Kalıcı kodlar arasında `INVALID_REQUEST`, `FORBIDDEN`, `PRECONDITION_FAILED`, `APPROVAL_REQUIRED`, `APPROVAL_EXPIRED`, `APPROVAL_MISMATCH`, `REVISION_CONFLICT`, `CAPABILITY_UNAVAILABLE`, `BUDGET_EXCEEDED`, `DEADLINE_EXCEEDED`, `IDEMPOTENCY_CONFLICT`, `CURSOR_EXPIRED` ve `OUTCOME_UNKNOWN` bulunur. Çeviri mesajı değiştirir; kodu değiştirmez.

Son süre, işe kabulü ve kesinti kontrolüne katılan yürütmeyi sınırlar; sürenin geçmesi etkilerin durduğunu kanıtlamaz. Kuyruk beklemesi, yürütme süresi ve iptalin etkili olduğu zaman ayrı gözlenir. Tarayıcının askıya alınması, sürücü zamanlaması ve dış çağrılar genel bir kesin gerçek zaman garantisini engeller.

## Yeniden deneme, iptal ve kurtarma

Değişiklikler özne, işlem ve kaynak kapsamında bir `idempotency_key` ile standart içerik hash'i kullanır. Yayımlanan saklama süresi içinde aynı tekrar mevcut sonucu/referansı döndürür. Aynı anahtarın farklı içerikle kullanılması `IDEMPOTENCY_CONFLICT` üretir. Ağ teslimi belirsizse yeni işlem oluşturmadan önce bilinen görev/yürütme sorgulanır veya aynı anahtarla tekrar yapılır. Saklama sınırı belirsizleşirse otomatik yeniden denemeler durdurulur.

Yalnızca açıkça yeniden denenebilir işaretlenen hatalar, bekleme süresine ve bütçeye uyarak tekrar edilir. Zaman aşımında etki tamamlanmış fakat yanıt alınamamış olabilir; uzlaştırılana kadar `OUTCOME_UNKNOWN` kaydedilir. Dış etkiler için genel bir tam olarak bir kez gerçekleşme iddiası kurulmaz.

İptal, geri alma değildir. `cancel_requested` isteğin alındığını gösterir; `cancelled`, kalan işin durduğuna ilişkin kanıt gerektirir. Tamamlanmış yürütme geç gelen iptale rağmen `succeeded` kalabilir. Kısmi etkiler kayıtta korunur. Telafi, düzeltilecek etkiye bağlanan yeni ve yetkili bir eylemdir. Model ağırlıklarını geri almak değişmiş veriyi veya dış eylemleri tersine çevirmez.

## Modeller, veri ve tam akış örneği

Model kaydı kaynak revizyonunu, dosya hash'lerini, mimariyi, operatörleri, nicemlemeyi ve motor uyumluluğunu sabitler. Davranış sözleşmesi gözlem, durum, eylem, zaman ve birimleri; politika/dünya arayüzlerini ve reset/taşıma kurallarını sabitler. İki dosyanın yüklenmesi anlam bakımından uyumlarını kanıtlamaz. Veri sürümleri kökeni ve izinli kullanımı korur; eğitim aday üretir, kendiliğinden sürüm yetkisi üretmez.

Örnek akışta yerel eylemci, sınırlı bir canary için aday model takımı önerir. `listCapabilities`, `createTask` ve `createActionProposal` kullanır; insan `getActionProposal` ile inceler. Politika HITL gerektiriyorsa `createApproval` tam öneriye bağlanır. `createExecution`, önerinin `transitionRelease` eylemini planlar. HOTL, `listEvents` ve `getExecution` ile izler; HIC maruz kalma kapsamını, bütçeyi ve yetki devrini yönetir. Kabul ölçütlerinin bozulması yetkili bir geri dönüş önerisi doğurur. Burada hiçbir aşamanın çalıştırıldığı iddia edilmez.

Zorunlu olumsuz örnekler; uyuşmayan içerik hash'i, süresi geçmiş onay, eski politika revizyonu, desteklenmeyen operatör ve tükenmiş bütçedir. Bunlar yetkisiz etki gerçekleşmeden başarısız olmalıdır. Çalışma sırasında kesinti ve bilinmeyen sonuç testleri ayrıca kısmi etkilerin doğru raporlandığını doğrular.

## Dokümantasyon ve kabul

İnsan belgeleri görev anlatımları, onay önizlemeleri, inceleme ve kurtarma rehberleri sağlar. AI belgeleri tam şemaları, etki kapsamlarını, önkoşulları, sınırları ve geçerli/geçersiz örnekleri sağlar. İkisi aynı revizyondan üretilir. Uygulanmış başlangıç rehberleri ancak eşleşen kod revizyonu, ortam, komutlar ve gözlenen sonuçlar bulunduğunda “test edilmiş örnek” olur.

Uygunluk kontrolleri üretilen arayüzleri karşılaştıracak, yetkilendirme ve tekrar anlamlarını çalıştıracak, kimliklerin diller arasında aynı kaldığını doğrulayacaktır. Yerel, tarayıcı ve GPU kanıtları ayrı tutulur; çalıştırılmayan kontroller `not_run` veya gerekçeli `not_applicable` kalır.

İlgili sözleşmeler: [mimari](../../framework-mimari-taslagi-v0.3.md), [takım protokolleri](../../ai-takim-protokolleri-v0.1.md), [öğrenme ve eğitim](../../ogrenme-verisi-ve-egitim-protokolu-v0.1.md), [NeedSpec şeması](../../ai-team-protocol/request.schema.json), [sentetik istek örneği](../../ai-team-protocol/request.example.json), [model takımı sürüm şeması](../../ai-team-protocol/model-set-release.schema.json). Şema doğrulaması yapıyı kontrol eder; yetki, referans bütünlüğü, uyumluluk ve çalıştırılmış kanıt bağımsız anlamsal kontroller gerektirir.
