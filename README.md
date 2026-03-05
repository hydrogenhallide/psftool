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
# 1. Make the template
psftool --new --width <width> --height <height> --glyphs <glyphs> <font>.bmp

# 2. Edit the .json file (optional)

# 3. Open <font>.bmp in GIMP, Krita, etc. and edit glyphs

# 4. Convert back
psftool <font>.bmp

# 5. Load into TTY
setfont <font>
```
## How do i edit an existing one?
```sh
# 1. Get a font from your system
cp <font path> <working directory>
# 1.5. Only if the file you have is a .psf.gz
gunzip -k <font>

# 2. Convert to BMP
psftool <font>

# 3. Open <font>.bmp in GIMP, Krita, etc. and edit glyphs

# 4. Convert back
psftool <font>.bmp

# 5. Load into TTY
setfont <font>
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
