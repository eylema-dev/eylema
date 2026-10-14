# Eyleme görsel kimliği

[English](README.md) · [Tüm paketi indir](../eylema-logo.zip)

![Eyleme görsel kimlik önizlemesi](brand-preview.png)

**E / eylem** işareti, Eyleme'nın baş harfini ileri yönelen bir orta kolla birleştirir. Eğik uçlar hareketi, yuvarlatılmış sol köşeler daha sıcak bir karakteri anlatır. Küçük harfli yazı logosu sade ve ulaşılabilir bir görünüm sağlar.

Ana renkler **eylem turuncusu `#EE5638`** ve **mürekkep `#17252B`**. Koyu zemindeki yazı beyazdır (`#FFFFFF`); sunum zemini kırık beyazdır (`#F7F6F2`).

- `logo.svg` / `logo.png`: açık zemin için yatay logo, şeffaf PNG 1600 piksel genişlikte.
- `logo-dark.svg` / `logo-dark.png`: koyu zemin için yatay logo, şeffaf PNG 1600 piksel genişlikte.
- `logo-mono.svg` / `logo-mono.png`: tek renk sürüm, şeffaf PNG 1600 piksel genişlikte.
- `logo-icon.svg` / `logo-icon.png`: tek başına işaret, şeffaf PNG 1024 × 1024.
- `github-avatar.svg` / `github-avatar.png`: koyu zeminli, dairesel kırpmaya uygun avatar; 1024 × 1024.
- `github-social.svg` / `github-social.png`: depo paylaşım görseli; 1280 × 640.
- `favicon-16.png`, `favicon-32.png`, `favicon-48.png`: küçük web simgeleri.
- `apple-touch-icon.png`: 180 × 180 dokunmatik cihaz simgesi.
- `brand-preview.svg` / `brand-preview.png`: aynı kaynak çizimlerin kullanım önizlemesi; 1600 × 1080.

SVG dosyaları düzenlenebilir kaynaktır; dış görsel, font, script, gradyan veya filtre içermez. Yazıda `'Helvetica Neue', Arial, sans-serif` kullanılır. SVG yazısının görünümü cihazdaki fonta göre değişebilir; PNG dosyaları üretilen görünümü korur. Font dosyası dağıtılmaz.

Oranları ve renkleri koruyun. İşaret çevresinde en az bir dikey gövde kalınlığı boşluk bırakın. Küçük boyutlarda tek başına işareti kullanın. Dairesel profil kırpması için avatar dosyasını seçin. Dosyanın depoda bulunması, hesap avatarı veya paylaşım ayarının değiştirildiğini tek başına kanıtlamaz.

Yeniden üretim: Node.js ve `sharp` bulunan ortamda depo kökünden `node tools/render-brand.cjs` çalıştırılır. İlk üretimde sharp 0.35.5 kullanıldı. Bağımlılıklar başka konumdaysa `NODE_PATH` o kurulumun `node_modules` dizinine ayarlanabilir. Değişiklikten sonra zip paketi bu klasörden yeniden oluşturulmalıdır.

Depo lisansı henüz seçilmedi. Bu paket marka kullanım lisansı veya marka tescili uygunluk beyanı değildir.
