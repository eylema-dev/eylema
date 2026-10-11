# Eylema — İsim ve uygunluk kaydı

[İngilizce](name-availability.md) kaynak belgedir; Türkçe ikinci dildir.

**Marka kararı:** Eylema adı proje sahibi tarafından 11 Ekim 2026'da kabul edildi. Görünen ürün adı **Eylema**. Önerilen CLI adı ve paket öneki `eylema`; bunlar tasarım kararlarıdır, yayımlanmış paket veya çalışan komut değildir.

**Kontrol:** 11 Ekim 2026, yaklaşık 15:49–15:56, Europe/Istanbul. Kayıt/API zaman damgası ve tarayıcı gözlemlerinin zaman aralığı [makinece okunabilir kayıtta](name-availability.json) bulunur. Uygunluk, kayıt yapılana kadar değişebilir.

**GitHub güncellemesi — 11 Ekim 2026:** [eylema-dev organizasyonu](https://github.com/eylema-dev) ve public [eylema-dev/eylema deposu](https://github.com/eylema-dev/eylema) artık oluşturuldu. Aşağıdaki sonuçlar, oluşturma öncesindeki uygunluk kontrolünün tarihsel kaydıdır. Güncel dokümantasyon adresi bu depodur; alan adı kaydı hâlâ beklemektedir.

## Alan adı sonuçları

| Aday | Kayıt kuruluşundaki gözlem | Sicil kontrolü |
|---|---|---|
| `eylema.com` | Namecheap kayıt için “Add to cart” düğmesi sundu; ek promosyonlar öncesi 10,98 USD/yıl gösterildi. | Verisign HTTP 404 döndürdü; ayrıştırılabilir RDAP hata nesnesi bulunmadığından sicil kanıtı kesin sonuç olarak değerlendirilmedi. |
| `eylema.dev` | Namecheap kayıt için “Add to cart” düğmesi sundu; 10,98 USD/yıl gösterildi. | Google Registry RDAP HTTP 404 / `errorCode: 404`: kayıt bulunamadı. |
| `eylema.ai` | Namecheap kayıt için “Add to cart” düğmesi sundu; 89,98 USD/yıl, en az iki yıl kayıt ve 114,98 USD/yıl yenileme bilgisi gösterildi. | Identity Digital RDAP HTTP 404 / `errorCode: 404`: kayıt bulunamadı. |

Kayıt kuruluşu kanıtı [Namecheap arama ekranından](https://www.namecheap.com/domains/registration/results/?domain=eylema.com) alındı: ilk `eylema.com` sorgusunda `eylema.ai` de listelendi; ardından arama alanında `eylema.dev` sorgulanarak kayıt teklifi görüldü. Fiyatlar arama ekranındaki gözlemlerdir, son ödeme tutarı değildir. Hiçbir alan adı sepete eklenmedi, satın alınmadı veya rezerve edilmedi.

Sicil uç noktaları [IANA RDAP listesi](https://data.iana.org/rdap/dns.json) üzerinden seçildi: [Verisign `.com`](https://rdap.verisign.com/com/v1/domain/eylema.com), [Google Registry `.dev`](https://pubapi.registry.google/rdap/domain/eylema.dev), [Identity Digital `.ai`](https://rdap.identitydigital.services/rdap/domain/eylema.ai). Tek başına RDAP kaydı bulunmaması satın alınabilirliği kanıtlamaz. Yukarıdaki kayıt teklifleri ayrıca canlı kontrol edildi.

## GitHub organizasyon sonuçları

| Aday | Açık ad alanı | Organizasyon adı formu |
|---|---|---|
| `eylema` | Mevcut [EylemA kullanıcı hesabı](https://github.com/EylemA) tarafından **kullanılıyor**; açık API HTTP 200 ve `type: User` döndürdü. | Mevcut hesap adı tuttuğu sürece yeni organizasyonda kullanılamaz. |
| `eylema-dev` | Açık kullanıcı API'si HTTP 404 / `Not Found` döndürdü. | Organizasyon kurulum formunun canlı ad denetimi başarılı oldu. Organizasyon oluşturulmadı veya rezerve edilmedi. |
| `eylema-platform` | Açık kullanıcı API'si HTTP 404 / `Not Found` döndürdü. | Organizasyon kurulum formunun canlı ad denetimi başarılı oldu. Organizasyon oluşturulmadı veya rezerve edilmedi. |

Kaynaklar: GitHub açık API'sinde [eylema](https://api.github.com/users/eylema), [eylema-dev](https://api.github.com/users/eylema-dev), [eylema-platform](https://api.github.com/users/eylema-platform) ve [organizasyon kurulum formu](https://github.com/organizations/new?plan=free). Form yalnızca ad alanını doğrulamak için kullanıldı; iletişim bilgileri, koşul kabulü veya oluşturma işlemi gönderilmedi. Başarılı ad denetimi rezervasyon değildir. GitHub'ın [kullanıcı adı politikası](https://docs.github.com/en/site-policy/other-site-policies/github-username-policy), dışarıdan etkin görünmeyen hesap adlarının da boş kabul edilemeyeceğini açıklar.

## İlk kontrol sırasında önerilen kamusal kimlik

- Marka: **Eylema** — kabul edildi.
- Ana proje ve dokümantasyon adresi: **`eylema.dev`** — öneri; bu çalışmada kaydedilmedi.
- GitHub organizasyonu: **`eylema-dev`** — öneri; ad denetimi geçti, oluşturulması ayrı bir talimat gerektiriyor.
- Gelecekteki ana depo: **`eylema-dev/eylema`** — önerilen yol, mevcut depo iddiası değil.
- `eylema.com` sonradan alınırsa ek marka adresi olarak kullanılabilir. `eylema.ai` isteğe bağlıdır.

İlk uygunluk kontrolü organizasyon veya alan adı sahipliği kanıtı değildi. Organizasyon ve depo için sonraki durum yukarıdaki güncellemede yer alır; alan adı sahipliği iddiası yoktur. Mevcut protokol kimlikleri, API işlem kimlikleri, dosya yolları ve sözleşme sürümleri korundu. Şemaya eşlik eden örnekteki `example/eylema` açıkça yer tutucudur. Bu kontrol marka tescili veya paket sicillerinde isim uygunluğu kontrolü değildir.
