# Eyleme ürün ailesi

Revizyon **1.0.0**, 15 Ekim 2026. Kanonik kaynak İngilizcedir; [English](../en/product-family.md). Bu belge bakımcının kabul ettiği ürün ve iş modelini kaydeder. Uygulama olgunluğu [çekirdek kanıtı](../../README.tr.md) ve her ürünün kabul kayıtlarıyla sınırlıdır.

## Tek marka, üç ürün

**Eyleme**, tek çatı marka ve bağımsız kullanılabilen açık kaynak topluluk ürünüdür. [eyleme.dev](https://eyleme.dev), başlangıç, belgeler, katkı ve ticari ürün tanıtımının ortak public giriş noktasıdır. Proje sahibi bu alan adını kaydettiğini bildirmiştir; yayın ve normal DNS/HTTPS erişimi ayrı canlı kanıt gerektirir.

| Ürün | Rol | Kaynak ve iş modeli |
|---|---|---|
| Eyleme | Açıkça yetkilendirilmiş görevleri çalıştırıp incelemek ve desteklenen eylemci iş akışlarını oluşturmak | Public açık kaynak ürün; kendi kaynak kodu ve belgeleri Apache-2.0 olarak kalır |
| Eyleme Derleme | Açık köken ve erişim sınırlarıyla veri toplamak, hazırlamak ve bütünleştirmek | Ayrı geliştirilen private kaynaklı, ücretli abonelikle sunulan ürün |
| Eyleme Onama | Açıkça bütünleştirilen iş akışları için politika ve yetkiyi değerlendirmek, yürütme güvencesi sağlamak | Ayrı geliştirilen private kaynaklı, ücretli abonelikle sunulan ürün |

Birlikte kullanım akışını **Derleme → Onama → Eyleme** olarak anlatın: veriyi hazırla, yetkiyi değerlendir, izin verilen işi yürüt. Bu bir ürün anlatımıdır; zorunlu runtime bağımlılık zinciri değildir. Politika kontrolleri toplama, planlama, yürütme ve açıklama öncesinde uygulanabilir. Ortak marka, abonelik veya Onama kararı örtük erişim, veri hakkı ya da yürütme yetkisi vermez.

## Bağımsız açık kaynak sözleşmesi

Desteklenen bağımsız Eyleme akışları Derleme veya Onama aboneliği, ücretli hesap, lisans sunucusu ya da ticari servis bağlantısı olmadan kullanılabilir kalmalıdır. Eyleme temel erişim denetimlerini, sınırlı yürütmeyi, açık retleri ve yerel güvenlik kontrollerini korur. Mevcut host doğrulamalı yetki, iptal ve kurtarma sınırları görünür kalır; bu karar bekleyen insan kimlik doğrulama veya üretim kapılarının geçildiğini iddia etmez.

Derleme ve Onama bütünleşmeleri isteğe bağlıdır ve açık, sürümlenmiş public arayüzleri kullanmalıdır. Ücretli ürünü kullanma hakkı; kaynak, cihaz, çalışma alanı veya proje üzerinde işlem yapma izninden ayrıdır. Her ürün kendi yetki ve güvenlik sınırını uygular; erişilemeyen ücretli bütünleşme kontrolleri sessizce aşmamalı veya ilgisiz, desteklenen bağımsız bir akışı engellememelidir.

## Topluluk ve ticari sınırlar

Public ekosistem yararlı örnekler, belgeler, issue tartışmaları, incelenmiş katkılar ve uyumlu uzantılar çevresinde gelişir. Public API/arayüz belgeleri, şemalar ve topluluk uzantı örnekleri belirtilen lisanslarıyla açık kalabilir; private uygulama depoları, müşteri ayarları, kimlik bilgileri ve işletme verileri topluluğa katılım önkoşulu değildir. [Katkı akışı](https://github.com/eyleme-dev/devguide/blob/main/CONTRIBUTING.tr.md) değişiklikleri ve bağımsız incelemeyi yönetmeye devam eder.

Derleme ve Onama'nın yeni ticari uygulamaları private geliştirilir ve ayrıca belirtilen koşullarla abonelik olarak sunulur. Depo görünürlüğü, daha önce yayımlanan kodu yeniden lisanslamaz ve mevcut Apache-2.0 izinlerini geri almaz. Önceden lisanslanmış bileşenlerin bildirimleri ve kökeni korunur, yeni ticari koddan ayrılır. Eyleme'nin Apache-2.0 lisansı aynı markayı taşıyan her ürüne kendiliğinden uygulanmaz. Kanonik [lisans ve katkı kapsamına](https://github.com/eyleme-dev/devguide/blob/main/docs/tr/licensing.md) bakın.

Ticari koşullar, fiyatlar ve abonelik erişilebilirliği henüz yayımlanmadı. Abonelik paketleri, faturalama, kullanım hakları, hizmet seviyeleri ve ticari erişilebilirlik kendi uygulama ve yayın kanıtlarını gerektirir. Bu model ödeme sayfası, ödeme bütünleşmesi veya genel kullanıma açık abonelik hizmeti oluşturmaz. Abonelik hosted dağıtımı zorunlu kılmaz; desteklenen yerel veya hosted sunum, gerçek kurulum ve veri sınırlarını açıklamalıdır.

## Adlandırma ve kanıt

Müşteriye yönelik ürün anlatımında **Eyleme**, **Eyleme Derleme** ve **Eyleme Onama** kullanılır. Ayrı bir taşıma açıkça değiştirmedikçe mevcut teknik adlar, komutlar, depo sahipliği, protokol/şema kimlikleri, ortam değişkenleri, kayıtlı veri yolları ve lisans bildirimleri korunur. Tarihsel `eylema.dev` adresi güncel topluluk hedefi değildir; mevcut kayıtları tarihçe olarak korunur.

Ticari genel bakış [sitenin ürün bölümünde](https://eyleme.dev/#products) public olarak sunulur; ürün ailesini anlamak private kaynak erişimi gerektirmez. Bu belge kabul edilen yönü tanımlar; depo gizliliği, site yayını, runtime yürütümü ve abonelik erişilebilirliği ayrı ayrı doğrulanması gereken iddialardır.
