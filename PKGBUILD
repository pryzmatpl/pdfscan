# PKGBUILD for Pdfium built against system ICU
# This builds Pdfium from source to match your system's ICU version
# Usage: makepkg -si

pkgname=pdfium-system-icu
pkgver=latest
pkgrel=1
pkgdesc="Pdfium library built against system ICU (compatible with ICU 78+)"
arch=('x86_64')
url="https://pdfium.googlesource.com/pdfium"
license=('BSD')
depends=('icu' 'freetype2' 'lcms2' 'openjpeg2' 'libjpeg-turbo' 'zlib')
makedepends=('git' 'python' 'ninja' 'gn' 'base-devel')
provides=('libpdfium')
conflicts=('pdfium-binaries')

prepare() {
  cd "$srcdir"
  
  # Install depot_tools first
  if [ ! -d "depot_tools" ]; then
    git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
  fi
  
  export PATH="$PATH:$srcdir/depot_tools"
  
  # Clone Pdfium if not exists
  if [ ! -d "pdfium" ]; then
    git clone https://pdfium.googlesource.com/pdfium
  fi
  
  cd pdfium
  git pull
  
  # Configure gclient
  if [ ! -f ".gclient" ]; then
    cat > .gclient << 'EOF'
solutions = [
  {
    "name": "pdfium",
    "url": "https://pdfium.googlesource.com/pdfium.git",
    "deps_file": "DEPS",
    "managed": False,
  },
]
EOF
  fi
  
  # Fetch dependencies
  echo "Fetching dependencies (this may take a while)..."
  gclient sync
}

build() {
  cd "$srcdir/pdfium"
  
  export PATH="$PATH:$srcdir/depot_tools"
  
  # Configure build to use system ICU and libraries
  mkdir -p out/Default
  cat > out/Default/args.gn << 'EOF'
is_debug = false
is_component_build = false
pdf_enable_xfa = false
pdf_enable_v8 = false
pdf_use_skia = false
pdf_use_skia_paths = false
use_system_icu = true
use_system_freetype = true
use_system_lcms2 = true
use_system_libjpeg = true
use_system_libpng = true
use_system_zlib = true
EOF
  
  # Generate build files
  echo "Generating build files..."
  gn gen out/Default
  
  # Build Pdfium
  echo "Building Pdfium (this will take 10-30 minutes)..."
  ninja -C out/Default pdfium
}

package() {
  cd "$srcdir/pdfium"
  
  # Install library
  install -Dm755 out/Default/libpdfium.so "$pkgdir/usr/lib/libpdfium.so"
  
  # Install headers (if needed for development)
  mkdir -p "$pkgdir/usr/include/pdfium"
  cp -r public/* "$pkgdir/usr/include/pdfium/" 2>/dev/null || true
  
  # Install license
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE" 2>/dev/null || true
}

