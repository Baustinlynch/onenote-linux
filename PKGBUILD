# Maintainer: onenote-linux contributors
# Unofficial Microsoft OneNote client for Linux. Not affiliated with Microsoft.

pkgname=onenote-linux
pkgver=0.2.0
pkgrel=1
pkgdesc='Unofficial Microsoft OneNote desktop client for Linux'
arch=('x86_64' 'aarch64')
url='https://github.com/Baustinlynch/onenote-linux'
license=('MIT')
depends=('webkit2gtk-4.1' 'gtk4' 'libayatana-appindicator' 'libnotify')
optdepends=('xdg-utils: open external links in the default browser')
makedepends=('cargo' 'rust')
provides=('onenote')
conflicts=('onenote-desktop')
source=("$pkgname-$pkgver.tar.gz::https://github.com/Baustinlynch/onenote-linux/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5')

build() {
    cd "$srcdir/$pkgname-$pkgver"
    # The frontend is static HTML in dist/; no Node toolchain is needed.
    cargo build --release --locked
}

package() {
    cd "$srcdir/$pkgname-$pkgver"
    install -Dm755 "target/release/onenote-linux" \
        "$pkgdir/usr/bin/onenote-linux"

    install -Dm644 "dist/onenote-linux.desktop" \
        "$pkgdir/usr/share/applications/onenote-linux.desktop"
    install -Dm644 "icons/128x128.png" \
        "$pkgdir/usr/share/icons/hicolor/128x128/apps/onenote-linux.png"
    install -Dm644 "icons/128x128@2x.png" \
        "$pkgdir/usr/share/icons/hicolor/128x128@2x/apps/onenote-linux.png"
    install -Dm644 "icons/32x32.png" \
        "$pkgdir/usr/share/icons/hicolor/32x32/apps/onenote-linux.png"
    install -Dm644 "icons/icon.svg" \
        "$pkgdir/usr/share/icons/hicolor/scalable/apps/onenote-linux.svg"

    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
