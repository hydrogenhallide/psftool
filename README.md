# psftool

Convert PSF bitmap fonts (used by the Linux TTY) to BMP images and back, so you can edit them in any image editor.

## Usage

```sh
# Convert a PSF font to an editable BMP
psftool font.psf

# Convert the edited BMP back to a PSF font
psftool font.bmp

# Create a blank font from scratch
psftool --new --width 16 --height 32 --glyphs 256 myfont.bmp
```

Output files are placed alongside the input. `font.psf` produces `font.bmp` and a `font.json` sidecar that stores metadata (dimensions, PSF version, flags). The sidecar is required for BMP → PSF conversion — don't delete it.

## BMP format

Glyphs are arranged in a grid of 16 per row. Each cell is separated by a 1px red border.

- **White** pixel = glyph bit ON (foreground)
- **Black** pixel = glyph bit OFF (background)
- **Red** pixel = cell separator (don't draw on these)

## So, how do i make a font?

```sh
# 1. Get a font from your system
cp /usr/share/consolefonts/Lat15-Fixed16.psf.gz /tmp/
gunzip /tmp/Lat15-Fixed16.psf.gz

# 2. Convert to BMP
psftool /tmp/Lat15-Fixed16.psf

# 3. Open /tmp/Lat15-Fixed16.bmp in GIMP, Krita, etc. and edit glyphs

# 4. Convert back
psftool /tmp/Lat15-Fixed16.bmp

# 5. Load into TTY
setfont /tmp/Lat15-Fixed16.psf
```

## Notes

- Supports PSF1 and PSF2. PSF1 is limited to 8px-wide glyphs.
- Unicode mapping tables (if present in the original font) are not preserved on round-trip. Glyph bitmaps are bit-perfect.
- For `.psf.gz` files, decompress first with `gunzip`.

## Building

```sh
cargo build --release
# binary at target/release/psftool
```
