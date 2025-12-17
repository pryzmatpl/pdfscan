#!/bin/bash
# Script to build Pdfium against system ICU for pdfscan project

set -e

BUILD_DIR="$HOME/.local/pdfium-build"
INSTALL_DIR="$HOME/.local/pdfium"

echo "Building Pdfium against system ICU..."
echo "This will take a while (10-30 minutes depending on your system)"

# Create build directory
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

# Clone Pdfium if not exists
if [ ! -d "pdfium" ]; then
    echo "Cloning Pdfium repository..."
    git clone https://pdfium.googlesource.com/pdfium
fi

cd pdfium
git pull

# Install depot_tools if needed
if [ ! -d "depot_tools" ]; then
    echo "Cloning depot_tools..."
    git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
fi

export PATH="$PATH:$BUILD_DIR/depot_tools"

# Configure gclient
cd pdfium
if [ ! -f ".gclient" ]; then
    echo "Configuring gclient..."
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
cd ..

# Configure build
echo "Configuring build..."
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
gn gen out/Default

# Build
echo "Building Pdfium (this will take 10-30 minutes)..."
ninja -C out/Default pdfium

# Install to local directory
echo "Installing to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR/lib"
cp out/Default/libpdfium.so "$INSTALL_DIR/lib/libpdfium.so"

echo ""
echo "Build complete! Pdfium library is at: $INSTALL_DIR/lib/libpdfium.so"
echo ""
echo "To use this library, you can either:"
echo "1. Copy it to /usr/lib: sudo cp $INSTALL_DIR/lib/libpdfium.so /usr/lib/"
echo "2. Set LD_LIBRARY_PATH: export LD_LIBRARY_PATH=$INSTALL_DIR/lib:\$LD_LIBRARY_PATH"
echo "3. Update your pdfium-render code to use bind_to_library() with the custom path"

