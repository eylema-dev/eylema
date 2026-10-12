# Yerel WebGPU sahnesi

G20, mevcut yerel görev konsoluna küçük bir tarayıcı WebGPU sahnesi ekler. Bu yalnızca görüntüleme amaçlı bir gösterimdir; görev yürütme, oturum yetkisi veya kalıcı görev durumuyla bağlantısı yoktur.

Yerel hizmeti yerel CLI ile başlatın ve yazdırılan `http://127.0.0.1:<port>/console` adresini WebGPU etkin bir tarayıcıda açın. Sayfa tarayıcı bağdaştırıcısı ve aygıtı ister, sahip olduğumuz WGSL gölgesini aynı loopback kaynağından yükler ve WebGPU tuvalinde sınırlandırılmış üç küp çizer. Bir küpe tıklayın veya tuvale odaklanıp ok tuşlarıyla nesneyi seçip taşıyın. Etiketli düğmeler aynı işlemleri sağlar.

`navigator.gpu` veya bağdaştırıcı kullanılamıyorsa sayfa `UNSUPPORTED` durumunu gösterir ve sahne kontrollerini devre dışı bırakır. Aygıtın `lost` sözü çözümlenirse `DEVICE_LOST` gösterilir, kontroller kapatılır ve tamponlar bırakılır. CPU veya WebGL çizimine geçiş yapılmaz. Sayfayı yenilemek yeni bir aygıt isteği başlatır. Belirleyici testteki aygıt kaybı benzetimdir; donanımda zorlanmış bir arıza değildir.

Sahne özellikle küçük tutulmuştur. Tek tarayıcı/M4 kabulü çapraz tarayıcı desteğini, WebGPU-Metal kaynak paylaşımını, kare süresini, enerji kullanımını veya model yürütmeyi kanıtlamaz. Görev konsolu davranışı sahneden bağımsızdır.
