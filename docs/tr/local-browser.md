# Yerel tarayıcı görev konsolu

Yerel macOS hizmeti, IPv4 loopback adresindeki `/console` yolunda aynı kaynaklı bir konsol sunar. `eylema service start` komutunun yazdırdığı tam adresi açıp sonuna `/console` ekleyin. Konsol sınırlandırılmış bir görev oluşturur, deterministik `addTaskValue` eylemini gönderir ve günlük destekli yerel host'tan kanonik görev/yürütme durumunu okur.

Sayfa, tarayıcının yerel depolamasında yalnızca görev ve yürütme kimliklerini ve seçilen dili tutar. Yeniden yükleme bu kimlikleri geri yükleyip hizmeti yeniden sorgular. CLI, port boş olduğu sürece hizmet yeniden başlatmalarında son loopback portunu korur; böylece aynı tarayıcı kaynağı kalıcı görevi kurtarabilir. Portu başka bir süreç kullanıyorsa hizmet güvenli biçimde yeni port seçer; tarayıcı depolaması kaynağa göre ayrıldığından yeni yazdırılan adresi açın. Hizmet geçici olarak kullanılamazsa kimlikler görünür kalır; bağlantı sağlanınca **Durumu yenile** ile host durumu tekrar okunur.

Tarayıcı, host tarafından verilen bearer değerini yalnızca konsol API yoluna kapsamlanmış `HttpOnly`, `SameSite=Strict` çerezinde alır. Değer URL'ye, sayfa betiğine, tarayıcı depolamasına, günlüklere veya iş günlüğüne yazılmaz. Değişiklik yapan istekler tam loopback `Host` ve `Origin`, same-origin fetch sinyali ve sayfanın CSRF belirtecini gerektirir. Yanıtlar sınırlandırılmış ve önbelleksizdir; sayfa üçüncü taraf varlık yüklemez.

Bu CLI oturumu doğrulanmış bir kişi değildir. Durdurma düğmesi normal typed iptal isteğini gönderir; host isteği `FORBIDDEN` ile reddeder ve arayüz gözlenen yürütme durumunu korur. Konsol işin durduğunu iddia etmez. Doğrulanmış insan kaydı ayrı ve güvenilir bir host işlemidir.

Tarayıcı arayüzü çalışma zamanı, model, GPU yolu, uzaktan kimlik doğrulama, TLS veya dağıtım eklemez. Yerel soket/günlük testleri ve gerçek tarayıcı oturumu ayrı kanıtlardır; tarayıcı çalışması GPU/model veya platformlar arası davranışı kanıtlamaz.
