# Building Pdfium Against System ICU

The system Pdfium library (`pdfium-binaries` from AUR) was compiled against ICU 76, but your system has ICU 78. This causes symbol lookup errors like `undefined symbol: u_isspace_76`.

## Solution: Build Pdfium from Source

Build Pdfium from source against your system's ICU version (78).

## Option 1: Using PKGBUILD (Recommended)

1. **Install build dependencies:**
   ```bash
   sudo pacman -S base-devel git python ninja gn icu freetype2 lcms2 openjpeg2 libjpeg-turbo zlib
   ```

2. **Build and install:**
   ```bash
   cd /nvme2/Workspace/pdfscan
   makepkg -si -f PKGBUILD
   ```

   This will:
   - Clone Pdfium from source
   - Build it against your system ICU (78)
   - Install it to `/usr/lib/libpdfium.so`
   - Replace the incompatible version

3. **Restart your application** - Pdfium should now work!

## Option 2: Using Build Script (Local Build)

If you prefer to build locally without installing system-wide:

1. **Run the build script:**
   ```bash
   cd /nvme2/Workspace/pdfscan
   ./build-pdfium.sh
   ```

2. **Copy the built library:**
   ```bash
   sudo cp ~/.local/pdfium/lib/libpdfium.so /usr/lib/libpdfium.so
   ```

   Or set `LD_LIBRARY_PATH`:
   ```bash
   export LD_LIBRARY_PATH=$HOME/.local/pdfium/lib:$LD_LIBRARY_PATH
   ```

## Option 3: Manual Build

If you want more control:

```bash
# Clone Pdfium
git clone https://pdfium.googlesource.com/pdfium
cd pdfium

# Get depot_tools
git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
export PATH="$PATH:$(pwd)/depot_tools"

# Fetch dependencies
gclient sync

# Configure build
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

# Build
gn gen out/Default
ninja -C out/Default pdfium

# Install
sudo cp out/Default/libpdfium.so /usr/lib/libpdfium.so
```

## Verification

After building, verify the library is linked against ICU 78:

```bash
ldd /usr/lib/libpdfium.so | grep icu
```

You should see `libicuuc.so.78` instead of `libicuuc.so.76`.

## Notes

- Building Pdfium takes 10-30 minutes depending on your CPU
- The build requires ~5-10GB of disk space
- The PKGBUILD approach is recommended as it integrates with pacman
- After building, the application will automatically use the new library

