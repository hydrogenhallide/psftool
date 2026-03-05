# PSF ↔ BMP Font Tool Design

## Overview

A Rust CLI utility (`psftool`) for converting PSF bitmap fonts (PSF1/PSF2) to editable BMP images and back. Makes TTY font creation and editing accessible via any image editor.

## Goals

- Convert PSF → BMP (editable grid of glyphs)
- Convert BMP → PSF (using sidecar JSON for metadata)
- Create blank BMP + sidecar for new font authoring
- Support PSF1 and PSF2
- Support arbitrary glyph sizes (higher-res fonts work naturally)

## Non-Goals

- Unicode table mapping (glyph order is preserved as-is)
- GUI
- Font rendering preview

## CLI Interface

Auto-detect by file extension:

```
psftool font.psf          # → font.bmp + font.json
psftool font.bmp          # → font.psf (requires font.json sidecar)
psftool --new --width 16 --height 32 --glyphs 256 --version 2 font.bmp
```

## BMP Format

- 24-bit RGB, no compression
- 16 glyphs per row
- Grid dimensions: `(width + 1) * 16` × `(height + 1) * ceil(num_glyphs / 16)` pixels
- Pixel colors:
  - Black `(0, 0, 0)` — glyph bit OFF
  - White `(255, 255, 255)` — glyph bit ON
  - Red `(255, 0, 0)` — cell separator (1px border around each glyph cell)

## Sidecar JSON (`font.json`)

```json
{
  "version": 2,
  "width": 8,
  "height": 16,
  "num_glyphs": 512,
  "flags": 0
}
```

Stored alongside the BMP. Required for BMP → PSF conversion.

## Architecture

Single binary, logic split across modules:

```
src/
  main.rs    — CLI entry, extension-based dispatch, --new mode
  psf.rs     — PSF1/PSF2 parse + serialize
  bmp.rs     — BMP read/write (hand-rolled, no image crate)
  meta.rs    — sidecar JSON read/write
```

### Dependencies

- `serde` + `serde_json` — JSON sidecar
- Nothing else (BMP is hand-rolled to avoid heavy deps)

## Data Flow

### PSF → BMP

1. Parse PSF header → extract width, height, num_glyphs, flags, version
2. Write `font.json` sidecar
3. Allocate BMP pixel buffer
4. For each glyph: unpack bitmap bits → place into grid cell with red border
5. Write BMP file

### BMP → PSF

1. Read `font.json` sidecar → dimensions + metadata
2. Parse BMP pixel buffer
3. For each grid cell: read pixels → pack into glyph bitmap
4. Serialize PSF1 or PSF2 based on `version` field

### --new Mode

1. Parse CLI flags (width, height, glyphs, version)
2. Write blank BMP (all black cells, red separators)
3. Write `font.json` sidecar

## Error Handling

`eprintln!` + `process::exit(1)` for all errors. No panics in normal operation.

Key error cases:
- Unknown file extension → clear message
- Missing sidecar on BMP → "missing font.json sidecar next to <file>"
- BMP dimensions don't match JSON → "BMP size doesn't match metadata"
- PSF1 with width > 8 → "PSF1 only supports 8px-wide glyphs"
- Corrupt/truncated PSF → descriptive parse error with byte offset

## Testing

- Unit tests per module (`#[cfg(test)]`): PSF round-trip, BMP grid math, JSON serde
- Integration test: known PSF → BMP → PSF, assert byte-identical output
- `cargo test` only, no additional framework
