# Leaximy

Leaximy, [Spotifast](https://github.com/crmne/spotifast) 0.11.2 üzerine kurulmuş,
lacivert–beyaz temalı, kehribar vurgulu kişisel bir Spotify istemcisidir.
Spotifast MIT lisanslıdır; `LICENSE` dosyası olduğu gibi korunmalıdır.

## Spotifast'tan farkları

| Konu | Spotifast | Leaximy |
| --- | --- | --- |
| Görünen ad, pencere başlığı, Spotify Connect cihaz adı | Spotifast | Leaximy |
| Komut | `spotifast` (+ `fastpotify`) | `leaximy` |
| Koyu tema | koyu gri + yeşil | lacivert + beyaz yazı + kehribar vurgu |
| Açık tema | beyaz + yeşil | beyaz + lacivert yazı + lacivert vurgu |
| İkon | yeşil disk, siyah üçgen | lacivert disk, beyaz üçgen, kehribar kenar |
| Mini oynatıcı (yerleşik Winamp teması) | koyu gri + yeşil, "SPOTIFAST" | lacivert + kehribar, "LEAXIMY" |
| Windows kurulumu | Spotifast kurulumu | `Leaximy-<sürüm>-Setup.exe`, Türkçe sihirbaz, ayrı uygulama kimliği |
| Ayar, önbellek, oturum (anahtar zinciri), D-Bus/MPRIS adları | `spotifast` | `leaximy` (Spotifast ile karışmaz, ikisi yan yana çalışır) |
| Otomatik güncelleme | GitHub'dan Spotifast sürümleri | açık: `atakangneyy/leaximy` sürümlerinden (kurulumla gelen kopyalar) |

Renkler `src/theme.rs` içindeki `Palette::dark()` ve `Palette::light()`
fonksiyonlarında, ikon `src/util.rs` içindeki `mark_rgba` fonksiyonunda,
mini oynatıcı teması `assets/skins/spotifast.wsz` dosyasında (üreticisi
`examples/default_skin.rs`).

## Windows kurulum dosyası (setup.exe)

Elle derlemene gerek yok. Bu depo GitHub'a gönderildiğinde
`.github/workflows/leaximy-windows.yml` her `main` güncellemesinde Windows
sürümünü derler ve **Releases** sayfasına iki dosya koyar:

- `Leaximy-<sürüm>-Setup.exe`: çift tıkla, "İleri"ye bas. Yönetici izni
  istemez, Başlat menüsüne ve masaüstüne kısayol koyar, Denetim Masası'ndan
  kaldırılabilir.
- `Leaximy-<sürüm>-portable.exe`: kurulumsuz, doğrudan çalışır.

Arkadaşların için Releases sayfasının bağlantısını paylaşman yeterli.
Kurulum imzasız olduğu için Windows SmartScreen ilk seferde "bilinmeyen
yayımcı" diyebilir: **Ek bilgi → Yine de çalıştır**.

Derleme yaklaşık 20–30 dakika sürer; ilerlemesi depodaki **Actions**
sekmesinde görünür. Elle başlatmak için: Actions → Leaximy Windows
installer → Run workflow.

## Derleme

Gerekenler: [rustup](https://rustup.rs) (doğru Rust sürümünü
`rust-toolchain.toml` dosyasından kendisi kurar), CMake, Clang.

- **Windows:** Visual Studio 2022 (C++ iş yükü), CMake, LLVM. MilkDrop
  görselleştiricisi için vcpkg ve `vcpkg install glew:x64-windows-static`
  gerekir; uğraşmak istemezsen MilkDrop'suz derle (aşağıda).
- **macOS:** `xcode-select --install`, `brew install cmake llvm`
- **Ubuntu/Debian:**
  `sudo apt install libasound2-dev libpulse-dev libxkbcommon-dev libwayland-dev cmake clang libclang-dev`

Sonra depo klasöründe:

```sh
cargo build --release                         # tam sürüm
cargo build --release --no-default-features   # MilkDrop olmadan, daha az bağımlılık
```

Uygulama `target/release/leaximy` (Windows'ta `leaximy.exe`) olarak çıkar.
Sisteme kurmak için: `cargo install --path .`

Çalmak için **Spotify Premium** gerekir; ücretsiz hesap gezinip arama yapabilir.

## Upstream'den yeni sürüm almak

```sh
git remote add upstream https://github.com/crmne/spotifast.git   # bir kez
git fetch upstream --tags
git rebase vX.Y.Z            # ya da: git merge vX.Y.Z
python3 leaximy/rebrand.py   # yeni gelen "Spotifast" adlarını tekrar çevirir
```

Çakışma çoğunlukla `src/theme.rs` ve `src/util.rs` dosyalarında çıkar:
oradaki Leaximy renklerini koru.

## Spotify girişi ve kullanıcı sınırı

Leaximy, Spotify'a kendi Spotify geliştirici uygulamasıyla bağlanır
(Client ID `src/auth.rs` içindeki `DEFAULT_WEB_CLIENT_ID`). Spotify'ın
geliştirme modu kuralları gereği:

- Uygulamanın [panelinde](https://developer.spotify.com/dashboard)
  **Redirect URIs** listesinde `http://127.0.0.1:8989/login` bulunmalı.
- Giriş yapabilecek herkes panelin **User Management** bölümüne adı ve
  Spotify e-postasıyla eklenmeli. Sahibi dahil en fazla 5 kişi.
- Listede olmayan biri giriş yapınca Spotify hata verir.

## Made for you ve öneriler

Spotify, Kasım 2024'ten sonra açılan uygulamalara kendi listelerini
(Discover Weekly, Release Radar, Daily Mix, daylist) aramada göstermiyor ve
öneri servisini kapatıyor. Leaximy bu yüzden:

- **Made for you** bölümünü, bu listelerden Spotify uygulamasında
  kütüphanene kaydettiklerinle doldurur (müzik çalan oturum üzerinden okunur).
  Kayıtlı liste yoksa bölüm görünmez.
- **Recommended for you** bölümü Spotify cevap vermediği için gizlenir.
- Arama ve sanatçı albümleri sayfa başına 10 sonuçla çalışır (geliştirme
  modu sınırı).

## Otomatik güncelleme

Setup.exe ile kurulan Leaximy, günde bir kez bu deponun en son sürümüne
(`v<sürüm>` etiketli) bakar, yenisini indirip doğrular ve "yeniden başlat"
denince sessizce kurar. Yeni sürüm yayınlamak için `Cargo.toml` ve
`Cargo.lock` içindeki sürümü artırıp `main`'e göndermek yeterli; iş akışı
gereken dosyaları (`leaximy-v<sürüm>-x86_64-pc-windows-msvc-setup.exe`,
`checksums.txt`) kendisi üretir. Taşınabilir exe kendini güncellemez.

## Tam ekran şarkı sözlerinde ritim halkası

Tam ekran şarkı sözleri görünümünde kapak yuvarlak bir disk olarak çizilir;
çevresindeki çubuklar çalan sesin spektrumunu gösterir (bas üstte, iki yana
aynalı) ve disk basla birlikte hafifçe büyür. Yalnızca bu bilgisayarda çalan
şarkıda hareket eder. Kod: `src/ui/lyrics.rs` içindeki `big_cover`.

## Masaüstü görselleştirici (Windows)

Oynatma çubuğundaki dizüstü bilgisayar simgesi görselleştiriciyi masaüstüne,
simgelerin arkasına koyar. Arka plan kullanıcının kendi duvar kâğıdıdır
(açılırken Windows'tan okunur). Solda söylenen satır büyük harflerle, kelime
kelime belirir; ortanın sağında yuvarlak kapak ve ritim halkası, sol altta
sıradaki dört şarkı durur. Sözler satır satır zamanlandığı için kelimelerin
anı satır süresine yayılarak tahmin edilir. Ana ekranı kaplar; Leaximy açık
(ya da simge durumunda) olduğu sürece görünür. Pencereyi `src/wallpaper.rs`
yerleştirir, içeriğini `src/ui/lyrics.rs` içindeki `wallpaper` çizer.
