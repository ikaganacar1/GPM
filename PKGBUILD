# Maintainer: Your Name <you@example.com>
pkgname=gpm
pkgver=0.1.0
pkgrel=1
pkgdesc="GPU & LLM Monitoring Service - Track NVIDIA GPU usage and Ollama sessions"
arch=('x86_64' 'aarch64')
url="https://github.com/ikaganacar1/GPM"
license=('MIT')
depends=('sqlite' 'nvidia-utils')
makedepends=('cargo' 'git')
options=(!lto)
source=("$pkgname::git+$url.git#tag=v$pkgver")
sha256sums=('SKIP')

prepare() {
    cd "$pkgname"
    cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$pkgname"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release --package gpm-core
}

package() {
    cd "$pkgname"

    # Install binaries
    install -Dm755 "target/$CARCH-unknown-linux-gnu/release/gpm" "$pkgdir/usr/bin/gpm"
    install -Dm755 "target/$CARCH-unknown-linux-gnu/release/gpm-server" "$pkgdir/usr/bin/gpm-server"

    # Install frontend
    install -dm755 "$pkgdir/usr/share/gpm"
    cp -r gpm-dashboard/dist/* "$pkgdir/usr/share/gpm/"

    # Install systemd services
    install -Dm644 " packaging/systemd/gpm.service" "$pkgdir/usr/lib/systemd/system/gpm.service"
    install -Dm644 "packaging/systemd/gpm-server.service" "$pkgdir/usr/lib/systemd/system/gpm-server.service"

    # Install default config
    install -Dm644 "config.example.toml" "$pkgdir/etc/gpm/config.toml.example"

    # Install man page (if exists)
    # install -Dm644 "docs/gpm.1" "$pkgdir/usr/share/man/man1/gpm.1"

    # Set up data directory
    install -dm755 "$pkgdir/var/lib/gpm"
}
