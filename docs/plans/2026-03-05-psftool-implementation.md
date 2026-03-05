# psftool Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust CLI that converts PSF bitmap fonts to editable BMP images and back, with sidecar JSON for metadata.

**Architecture:** Single binary with four modules: `psf.rs` (parse/serialize PSF1+PSF2), `bmp.rs` (hand-rolled 24-bit BMP read/write), `meta.rs` (sidecar JSON), and `main.rs` (CLI dispatch). Auto-detects direction by file extension. No image crates — BMP is simple enough to hand-roll.

**Tech Stack:** Rust, `serde` + `serde_json` for JSON, standard library only otherwise.

---

## PSF Format Reference

**PSF1** (legacy):
- Magic: `[0x36, 0x04]`
- Mode byte: bit 0 = 512 glyphs (else 256), bit 1 = has unicode table
- Charsize byte: bytes per glyph (= height; width always 8)
- Glyph data: `num_glyphs * charsize` bytes

**PSF2** (modern):
- Magic: `[0x72, 0xb5, 0x4a, 0x86]`
- Then 7 u32 LE fields: version(0), headersize(32), flags, numglyph, bytesperglyph, height, width
- `bytesperglyph = height * ceil(width / 8)`
- Glyph data: `numglyph * bytesperglyph` bytes

## BMP Grid Layout

- 16 glyphs per row
- Each glyph cell: `glyph_width` × `glyph_height` pixels
- 1px red `(255, 0, 0)` separators on all sides (outer border + between cells)
- BMP width: `1 + 16 * (glyph_width + 1)` px
- BMP height: `1 + num_rows * (glyph_height + 1)` px, where `num_rows = (num_glyphs + 15) / 16`
- Glyph `i` starts at pixel: `x = 1 + (i % 16) * (glyph_width + 1)`, `y = 1 + (i / 16) * (glyph_height + 1)`
- Pixel colors: white `(255,255,255)` = bit ON, black `(0,0,0)` = bit OFF

## BMP File Format (24-bit, top-down)

File header (14 bytes):
- `"BM"` magic (2 bytes)
- File size u32 LE
- Reserved `0u32` LE
- Pixel data offset u32 LE = `54`

DIB header / BITMAPINFOHEADER (40 bytes):
- Header size u32 LE = `40`
- Width i32 LE
- Height i32 LE = **negative** (top-down, no row reversal needed)
- Planes u16 LE = `1`
- Bits per pixel u16 LE = `24`
- Compression u32 LE = `0`
- Image size u32 LE = `0` (ok for uncompressed)
- X pixels/meter i32 LE = `2835`
- Y pixels/meter i32 LE = `2835`
- Colors used u32 LE = `0`
- Colors important u32 LE = `0`

Pixel data: `width * 3` bytes per row (BGR order), each row **padded to 4-byte boundary** with zero bytes. Rows top-to-bottom (because negative height).

Row stride = `(width * 3 + 3) & !3`

---

## Task 1: Scaffold project

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/psf.rs`
- Create: `src/bmp.rs`
- Create: `src/meta.rs`

**Step 1: Create the project**

```bash
cd /home/user/claude/psftool
cargo init --name psftool
```

**Step 2: Set dependencies in `Cargo.toml`**

```toml
[package]
name = "psftool"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**Step 3: Create module stubs**

`src/meta.rs`:
```rust
// sidecar JSON
```

`src/bmp.rs`:
```rust
// BMP read/write
```

`src/psf.rs`:
```rust
// PSF1/PSF2 parse and serialize
```

**Step 4: Wire modules in `src/main.rs`**

```rust
mod meta;
mod bmp;
mod psf;

fn main() {}
```

**Step 5: Verify it compiles**

```bash
cargo build
```
Expected: success (warnings OK).

---

## Task 2: meta.rs — FontMeta struct

**Files:**
- Modify: `src/meta.rs`
- Test: inline `#[cfg(test)]` in `src/meta.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn round_trip_json() {
        let meta = FontMeta { version: 2, width: 8, height: 16, num_glyphs: 512, flags: 0 };
        let tmp = std::env::temp_dir().join("psftool_test_meta.json");
        meta.save(&tmp).unwrap();
        let loaded = FontMeta::load(&tmp).unwrap();
        assert_eq!(loaded.version, 2);
        assert_eq!(loaded.width, 8);
        assert_eq!(loaded.height, 16);
        assert_eq!(loaded.num_glyphs, 512);
        assert_eq!(loaded.flags, 0);
        std::fs::remove_file(&tmp).ok();
    }
}
```

**Step 2: Run to verify it fails**

```bash
cargo test meta
```
Expected: compile error (FontMeta not defined).

**Step 3: Implement**

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct FontMeta {
    pub version: u8,
    pub width: u32,
    pub height: u32,
    pub num_glyphs: u32,
    pub flags: u32,
}

impl FontMeta {
    pub fn load(path: &Path) -> Result<Self, String> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
        serde_json::from_str(&data)
            .map_err(|e| format!("invalid JSON in {}: {}", path.display(), e))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("JSON error: {}", e))?;
        std::fs::write(path, data)
            .map_err(|e| format!("cannot write {}: {}", path.display(), e))
    }
}
```

**Step 4: Run tests**

```bash
cargo test meta
```
Expected: PASS.

---

## Task 3: bmp.rs — BMP writer

**Files:**
- Modify: `src/bmp.rs`
- Test: inline in `src/bmp.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_read_pixel() {
        // 3x2 image: top-left red, rest black
        let width = 3usize;
        let height = 2usize;
        let mut pixels = vec![[0u8; 3]; width * height]; // [R, G, B]
        pixels[0] = [255, 0, 0];

        let tmp = std::env::temp_dir().join("psftool_test.bmp");
        write_bmp(&tmp, width, height, &pixels).unwrap();
        let (rw, rh, rpix) = read_bmp(&tmp).unwrap();
        assert_eq!(rw, width);
        assert_eq!(rh, height);
        assert_eq!(rpix[0], [255, 0, 0]);
        assert_eq!(rpix[1], [0, 0, 0]);
        std::fs::remove_file(&tmp).ok();
    }
}
```

**Step 2: Run to verify it fails**

```bash
cargo test bmp::tests::write_and_read_pixel
```
Expected: compile error.

**Step 3: Implement the writer**

```rust
use std::path::Path;

pub fn write_bmp(path: &Path, width: usize, height: usize, pixels: &[[u8; 3]]) -> Result<(), String> {
    assert_eq!(pixels.len(), width * height);
    let stride = (width * 3 + 3) & !3;
    let pixel_data_size = stride * height;
    let file_size = 54 + pixel_data_size;

    let mut buf: Vec<u8> = Vec::with_capacity(file_size);

    // File header
    buf.extend_from_slice(b"BM");
    buf.extend_from_slice(&(file_size as u32).to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // reserved
    buf.extend_from_slice(&54u32.to_le_bytes()); // pixel offset

    // DIB header (BITMAPINFOHEADER)
    buf.extend_from_slice(&40u32.to_le_bytes());                   // header size
    buf.extend_from_slice(&(width as i32).to_le_bytes());          // width
    buf.extend_from_slice(&(-(height as i32)).to_le_bytes());      // height (negative = top-down)
    buf.extend_from_slice(&1u16.to_le_bytes());                    // planes
    buf.extend_from_slice(&24u16.to_le_bytes());                   // bpp
    buf.extend_from_slice(&0u32.to_le_bytes());                    // compression
    buf.extend_from_slice(&0u32.to_le_bytes());                    // image size (0 ok)
    buf.extend_from_slice(&2835i32.to_le_bytes());                 // x ppm
    buf.extend_from_slice(&2835i32.to_le_bytes());                 // y ppm
    buf.extend_from_slice(&0u32.to_le_bytes());                    // colors used
    buf.extend_from_slice(&0u32.to_le_bytes());                    // colors important

    // Pixel data (top-down, BGR order, row-padded)
    for row in 0..height {
        let mut row_bytes = Vec::with_capacity(stride);
        for col in 0..width {
            let [r, g, b] = pixels[row * width + col];
            row_bytes.push(b);
            row_bytes.push(g);
            row_bytes.push(r);
        }
        while row_bytes.len() < stride {
            row_bytes.push(0);
        }
        buf.extend_from_slice(&row_bytes);
    }

    std::fs::write(path, &buf)
        .map_err(|e| format!("cannot write {}: {}", path.display(), e))
}
```

**Step 4: Implement the reader**

```rust
pub fn read_bmp(path: &Path) -> Result<(usize, usize, Vec<[u8; 3]>), String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;

    if data.len() < 54 {
        return Err("file too small to be a BMP".to_string());
    }
    if &data[0..2] != b"BM" {
        return Err("not a BMP file".to_string());
    }

    let pixel_offset = u32::from_le_bytes(data[10..14].try_into().unwrap()) as usize;
    let width = i32::from_le_bytes(data[18..22].try_into().unwrap());
    let height_raw = i32::from_le_bytes(data[22..26].try_into().unwrap());
    let bpp = u16::from_le_bytes(data[28..30].try_into().unwrap());
    let compression = u32::from_le_bytes(data[30..34].try_into().unwrap());

    if bpp != 24 {
        return Err(format!("unsupported BMP bpp: {} (expected 24)", bpp));
    }
    if compression != 0 {
        return Err("compressed BMP not supported".to_string());
    }

    let top_down = height_raw < 0;
    let height = height_raw.unsigned_abs() as usize;
    let width = width as usize;
    let stride = (width * 3 + 3) & !3;

    if data.len() < pixel_offset + stride * height {
        return Err("BMP pixel data truncated".to_string());
    }

    let mut pixels = vec![[0u8; 3]; width * height];
    for row in 0..height {
        let src_row = if top_down { row } else { height - 1 - row };
        let row_start = pixel_offset + src_row * stride;
        for col in 0..width {
            let b = data[row_start + col * 3];
            let g = data[row_start + col * 3 + 1];
            let r = data[row_start + col * 3 + 2];
            pixels[row * width + col] = [r, g, b];
        }
    }

    Ok((width, height, pixels))
}
```

**Step 5: Run tests**

```bash
cargo test bmp
```
Expected: PASS.

---

## Task 4: psf.rs — PSF parse

**Files:**
- Modify: `src/psf.rs`
- Test: inline in `src/psf.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_psf2(width: u32, height: u32, num_glyphs: u32, glyph_byte: u8) -> Vec<u8> {
        let bytes_per_glyph = height * ((width + 7) / 8);
        let mut data = vec![];
        data.extend_from_slice(&[0x72, 0xb5, 0x4a, 0x86]); // magic
        data.extend_from_slice(&0u32.to_le_bytes());           // version
        data.extend_from_slice(&32u32.to_le_bytes());          // headersize
        data.extend_from_slice(&0u32.to_le_bytes());           // flags
        data.extend_from_slice(&num_glyphs.to_le_bytes());
        data.extend_from_slice(&bytes_per_glyph.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        data.extend_from_slice(&width.to_le_bytes());
        for _ in 0..(num_glyphs * bytes_per_glyph) {
            data.push(glyph_byte);
        }
        data
    }

    #[test]
    fn parse_psf2_basic() {
        let data = make_psf2(8, 16, 256, 0xAA);
        let font = parse(&data).unwrap();
        assert_eq!(font.meta.version, 2);
        assert_eq!(font.meta.width, 8);
        assert_eq!(font.meta.height, 16);
        assert_eq!(font.meta.num_glyphs, 256);
        assert_eq!(font.glyphs.len(), 256);
        assert_eq!(font.glyphs[0].len(), 16); // 16 bytes per glyph (8px wide = 1 byte/row)
        assert_eq!(font.glyphs[0][0], 0xAA);
    }

    #[test]
    fn parse_psf1_basic() {
        let mut data = vec![0x36, 0x04]; // magic
        data.push(0x00);                  // mode: 256 glyphs
        data.push(16);                    // charsize = height = 16
        for _ in 0..(256 * 16) { data.push(0xFF); }
        let font = parse(&data).unwrap();
        assert_eq!(font.meta.version, 1);
        assert_eq!(font.meta.width, 8);
        assert_eq!(font.meta.height, 16);
        assert_eq!(font.meta.num_glyphs, 256);
        assert_eq!(font.glyphs[0][0], 0xFF);
    }
}
```

**Step 2: Run to verify it fails**

```bash
cargo test psf::tests
```
Expected: compile error.

**Step 3: Implement**

```rust
use crate::meta::FontMeta;

const PSF1_MAGIC: [u8; 2] = [0x36, 0x04];
const PSF2_MAGIC: [u8; 4] = [0x72, 0xb5, 0x4a, 0x86];

pub struct PsfFont {
    pub meta: FontMeta,
    /// Raw glyph bitmaps. Each entry is `bytes_per_glyph` bytes.
    /// Bits are MSB-first; row r, col c is bit (7 - c) of byte (r * bytes_per_row + c/8).
    pub glyphs: Vec<Vec<u8>>,
}

pub fn parse(data: &[u8]) -> Result<PsfFont, String> {
    if data.starts_with(&PSF2_MAGIC) {
        parse_psf2(data)
    } else if data.starts_with(&PSF1_MAGIC) {
        parse_psf1(data)
    } else {
        Err("not a PSF font (unrecognized magic bytes)".to_string())
    }
}

fn parse_psf1(data: &[u8]) -> Result<PsfFont, String> {
    if data.len() < 4 {
        return Err("PSF1 header truncated".to_string());
    }
    let mode = data[2];
    let charsize = data[3] as usize;
    let num_glyphs: usize = if mode & 0x01 != 0 { 512 } else { 256 };
    let expected = 4 + num_glyphs * charsize;
    if data.len() < expected {
        return Err(format!("PSF1 data truncated: need {} bytes, got {}", expected, data.len()));
    }
    let mut glyphs = Vec::with_capacity(num_glyphs);
    for i in 0..num_glyphs {
        glyphs.push(data[4 + i * charsize .. 4 + (i + 1) * charsize].to_vec());
    }
    Ok(PsfFont {
        meta: FontMeta { version: 1, width: 8, height: charsize as u32, num_glyphs: num_glyphs as u32, flags: mode as u32 },
        glyphs,
    })
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    data.get(offset..offset + 4)
        .ok_or_else(|| format!("truncated at offset {}", offset))
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

fn parse_psf2(data: &[u8]) -> Result<PsfFont, String> {
    if data.len() < 32 {
        return Err("PSF2 header truncated".to_string());
    }
    let headersize  = read_u32(data, 8)? as usize;
    let flags       = read_u32(data, 12)?;
    let num_glyphs  = read_u32(data, 16)? as usize;
    let bytes_per_g = read_u32(data, 20)? as usize;
    let height      = read_u32(data, 24)?;
    let width       = read_u32(data, 28)?;

    let expected = headersize + num_glyphs * bytes_per_g;
    if data.len() < expected {
        return Err(format!("PSF2 data truncated: need {} bytes, got {}", expected, data.len()));
    }
    let mut glyphs = Vec::with_capacity(num_glyphs);
    for i in 0..num_glyphs {
        let start = headersize + i * bytes_per_g;
        glyphs.push(data[start..start + bytes_per_g].to_vec());
    }
    Ok(PsfFont {
        meta: FontMeta { version: 2, width, height, num_glyphs: num_glyphs as u32, flags },
        glyphs,
    })
}
```

**Step 4: Run tests**

```bash
cargo test psf::tests
```
Expected: PASS.

---

## Task 5: psf.rs — PSF serialize

**Files:**
- Modify: `src/psf.rs`
- Test: add to existing `#[cfg(test)]` block

**Step 1: Write the failing test**

```rust
#[test]
fn serialize_psf2_round_trip() {
    let data = make_psf2(8, 16, 256, 0xAA);
    let font = parse(&data).unwrap();
    let out = serialize(&font).unwrap();
    assert_eq!(out, data);
}

#[test]
fn serialize_psf1_round_trip() {
    let mut data = vec![0x36, 0x04, 0x00, 16u8];
    for _ in 0..(256 * 16) { data.push(0xBB); }
    let font = parse(&data).unwrap();
    let out = serialize(&font).unwrap();
    assert_eq!(out, data);
}
```

**Step 2: Run to verify it fails**

```bash
cargo test psf::tests::serialize
```
Expected: compile error (serialize not defined).

**Step 3: Implement**

```rust
pub fn serialize(font: &PsfFont) -> Result<Vec<u8>, String> {
    match font.meta.version {
        1 => serialize_psf1(font),
        2 => serialize_psf2(font),
        v => Err(format!("unknown PSF version: {}", v)),
    }
}

fn serialize_psf1(font: &PsfFont) -> Result<Vec<u8>, String> {
    if font.meta.width != 8 {
        return Err(format!("PSF1 only supports 8px-wide glyphs, got {}", font.meta.width));
    }
    let charsize = font.meta.height as usize;
    let mut out = vec![0x36, 0x04, font.meta.flags as u8, charsize as u8];
    for glyph in &font.glyphs {
        out.extend_from_slice(glyph);
    }
    Ok(out)
}

fn serialize_psf2(font: &PsfFont) -> Result<Vec<u8>, String> {
    let bytes_per_glyph = (font.meta.height * ((font.meta.width + 7) / 8)) as usize;
    let mut out = Vec::new();
    out.extend_from_slice(&PSF2_MAGIC);
    out.extend_from_slice(&0u32.to_le_bytes());                               // version
    out.extend_from_slice(&32u32.to_le_bytes());                              // headersize
    out.extend_from_slice(&font.meta.flags.to_le_bytes());
    out.extend_from_slice(&font.meta.num_glyphs.to_le_bytes());
    out.extend_from_slice(&(bytes_per_glyph as u32).to_le_bytes());
    out.extend_from_slice(&font.meta.height.to_le_bytes());
    out.extend_from_slice(&font.meta.width.to_le_bytes());
    for glyph in &font.glyphs {
        if glyph.len() != bytes_per_glyph {
            return Err(format!("glyph size mismatch: expected {}, got {}", bytes_per_glyph, glyph.len()));
        }
        out.extend_from_slice(glyph);
    }
    Ok(out)
}
```

**Step 4: Run tests**

```bash
cargo test psf
```
Expected: all PASS.

---

## Task 6: Conversion helpers — glyphs ↔ pixels

These are pure functions used by both directions of conversion. Put them in `main.rs` or a small `convert.rs` — using `main.rs` here to keep it simple.

**Files:**
- Modify: `src/main.rs`
- Test: inline in `src/main.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_to_pixels_full() {
        // 8x2 glyph, first row all-on, second row all-off
        let glyph = vec![0xFF, 0x00];
        let pixels = glyph_to_pixels(&glyph, 8, 2);
        for col in 0..8 { assert_eq!(pixels[col], [255, 255, 255]); }
        for col in 0..8 { assert_eq!(pixels[8 + col], [0, 0, 0]); }
    }

    #[test]
    fn pixels_to_glyph_round_trip() {
        let glyph = vec![0b10101010u8, 0b01010101u8];
        let pixels = glyph_to_pixels(&glyph, 8, 2);
        let back = pixels_to_glyph(&pixels, 8, 2);
        assert_eq!(back, glyph);
    }
}
```

**Step 2: Run to verify it fails**

```bash
cargo test tests::glyph
```
Expected: compile error.

**Step 3: Implement**

```rust
/// Convert raw glyph bitmap to RGB pixels (width * height entries, row-major).
fn glyph_to_pixels(glyph: &[u8], width: u32, height: u32) -> Vec<[u8; 3]> {
    let bytes_per_row = ((width + 7) / 8) as usize;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for row in 0..height as usize {
        for col in 0..width as usize {
            let byte = glyph[row * bytes_per_row + col / 8];
            let bit = (byte >> (7 - (col % 8))) & 1;
            pixels.push(if bit != 0 { [255u8, 255, 255] } else { [0u8, 0, 0] });
        }
    }
    pixels
}

/// Convert RGB pixels back to raw glyph bitmap.
/// A pixel is ON if it's closer to white than black (R+G+B > 382).
fn pixels_to_glyph(pixels: &[[u8; 3]], width: u32, height: u32) -> Vec<u8> {
    let bytes_per_row = ((width + 7) / 8) as usize;
    let mut glyph = vec![0u8; height as usize * bytes_per_row];
    for row in 0..height as usize {
        for col in 0..width as usize {
            let [r, g, b] = pixels[row * width as usize + col];
            let on = (r as u32 + g as u32 + b as u32) > 382;
            if on {
                glyph[row * bytes_per_row + col / 8] |= 1 << (7 - (col % 8));
            }
        }
    }
    glyph
}
```

**Step 4: Run tests**

```bash
cargo test tests::glyph
```
Expected: PASS.

---

## Task 7: main.rs — PSF → BMP

**Files:**
- Modify: `src/main.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn psf_to_bmp_creates_files() {
    use std::path::Path;
    // Build a minimal PSF2 font (2 glyphs, 8x2)
    let mut psf_data = vec![];
    psf_data.extend_from_slice(&[0x72, 0xb5, 0x4a, 0x86]);
    psf_data.extend_from_slice(&0u32.to_le_bytes());
    psf_data.extend_from_slice(&32u32.to_le_bytes());
    psf_data.extend_from_slice(&0u32.to_le_bytes());
    psf_data.extend_from_slice(&2u32.to_le_bytes()); // 2 glyphs
    psf_data.extend_from_slice(&2u32.to_le_bytes()); // 2 bytes per glyph
    psf_data.extend_from_slice(&2u32.to_le_bytes()); // height 2
    psf_data.extend_from_slice(&8u32.to_le_bytes()); // width 8
    psf_data.extend_from_slice(&[0xFF, 0x00]); // glyph 0
    psf_data.extend_from_slice(&[0x00, 0xFF]); // glyph 1

    let tmp_dir = std::env::temp_dir();
    let psf_path = tmp_dir.join("test_font.psf");
    let bmp_path = tmp_dir.join("test_font.bmp");
    let json_path = tmp_dir.join("test_font.json");

    std::fs::write(&psf_path, &psf_data).unwrap();
    convert_psf_to_bmp(&psf_path).unwrap();

    assert!(bmp_path.exists(), "BMP not created");
    assert!(json_path.exists(), "JSON not created");

    std::fs::remove_file(&psf_path).ok();
    std::fs::remove_file(&bmp_path).ok();
    std::fs::remove_file(&json_path).ok();
}
```

**Step 2: Run to verify it fails**

```bash
cargo test tests::psf_to_bmp
```
Expected: compile error.

**Step 3: Implement `convert_psf_to_bmp`**

```rust
fn convert_psf_to_bmp(psf_path: &std::path::Path) -> Result<(), String> {
    let data = std::fs::read(psf_path)
        .map_err(|e| format!("cannot read {}: {}", psf_path.display(), e))?;
    let font = psf::parse(&data)?;

    let w = font.meta.width as usize;
    let h = font.meta.height as usize;
    let n = font.meta.num_glyphs as usize;
    let cols = 16usize;
    let rows = (n + cols - 1) / cols;

    let bmp_w = 1 + cols * (w + 1);
    let bmp_h = 1 + rows * (h + 1);
    let red = [255u8, 0, 0];

    let mut pixels = vec![[0u8; 3]; bmp_w * bmp_h];

    // Fill all separator pixels red
    for y in 0..bmp_h {
        for x in 0..bmp_w {
            let on_vsep = x % (w + 1) == 0;
            let on_hsep = y % (h + 1) == 0;
            if on_vsep || on_hsep {
                pixels[y * bmp_w + x] = red;
            }
        }
    }

    // Place each glyph
    for (i, glyph) in font.glyphs.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let x0 = 1 + col * (w + 1);
        let y0 = 1 + row * (h + 1);
        let glyph_pixels = glyph_to_pixels(glyph, font.meta.width, font.meta.height);
        for gy in 0..h {
            for gx in 0..w {
                pixels[(y0 + gy) * bmp_w + (x0 + gx)] = glyph_pixels[gy * w + gx];
            }
        }
    }

    let stem = psf_path.with_extension("");
    let bmp_path = stem.with_extension("bmp");
    let json_path = stem.with_extension("json");

    bmp::write_bmp(&bmp_path, bmp_w, bmp_h, &pixels)?;
    font.meta.save(&json_path)?;

    println!("wrote {} and {}", bmp_path.display(), json_path.display());
    Ok(())
}
```

**Step 4: Run tests**

```bash
cargo test tests::psf_to_bmp
```
Expected: PASS.

---

## Task 8: main.rs — BMP → PSF

**Files:**
- Modify: `src/main.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn bmp_to_psf_round_trip() {
    // Build PSF2, convert to BMP, convert back, compare glyph data
    let mut psf_data = vec![];
    psf_data.extend_from_slice(&[0x72, 0xb5, 0x4a, 0x86]);
    psf_data.extend_from_slice(&0u32.to_le_bytes());
    psf_data.extend_from_slice(&32u32.to_le_bytes());
    psf_data.extend_from_slice(&0u32.to_le_bytes());
    psf_data.extend_from_slice(&4u32.to_le_bytes()); // 4 glyphs
    psf_data.extend_from_slice(&2u32.to_le_bytes()); // 2 bytes per glyph
    psf_data.extend_from_slice(&2u32.to_le_bytes()); // height 2
    psf_data.extend_from_slice(&8u32.to_le_bytes()); // width 8
    psf_data.extend_from_slice(&[0xFF, 0x00]);
    psf_data.extend_from_slice(&[0xAA, 0x55]);
    psf_data.extend_from_slice(&[0x0F, 0xF0]);
    psf_data.extend_from_slice(&[0x00, 0xFF]);

    let tmp_dir = std::env::temp_dir();
    let psf_in  = tmp_dir.join("rt_font.psf");
    let bmp_path = tmp_dir.join("rt_font.bmp");
    let json_path = tmp_dir.join("rt_font.json");
    let psf_out = tmp_dir.join("rt_font_out.psf");

    std::fs::write(&psf_in, &psf_data).unwrap();
    convert_psf_to_bmp(&psf_in).unwrap();
    convert_bmp_to_psf(&bmp_path).unwrap();

    let result = std::fs::read(&psf_out).unwrap();
    // Headers should match; compare glyph bytes (offset 32 onward)
    assert_eq!(&result[32..], &psf_data[32..]);

    for p in [&psf_in, &bmp_path, &json_path, &psf_out] { std::fs::remove_file(p).ok(); }
}
```

**Step 2: Run to verify it fails**

```bash
cargo test tests::bmp_to_psf
```
Expected: compile error.

**Step 3: Implement `convert_bmp_to_psf`**

```rust
fn convert_bmp_to_psf(bmp_path: &std::path::Path) -> Result<(), String> {
    let json_path = bmp_path.with_extension("json");
    if !json_path.exists() {
        return Err(format!(
            "missing sidecar: {} (needed for BMP→PSF conversion)",
            json_path.display()
        ));
    }

    let meta = meta::FontMeta::load(&json_path)?;
    let (bmp_w, bmp_h, pixels) = bmp::read_bmp(bmp_path)?;

    let w = meta.width as usize;
    let h = meta.height as usize;
    let n = meta.num_glyphs as usize;
    let cols = 16usize;

    let expected_w = 1 + cols * (w + 1);
    let expected_h = 1 + ((n + cols - 1) / cols) * (h + 1);
    if bmp_w != expected_w || bmp_h != expected_h {
        return Err(format!(
            "BMP size {}x{} doesn't match metadata (expected {}x{})",
            bmp_w, bmp_h, expected_w, expected_h
        ));
    }

    let mut glyphs = Vec::with_capacity(n);
    for i in 0..n {
        let col = i % cols;
        let row = i / cols;
        let x0 = 1 + col * (w + 1);
        let y0 = 1 + row * (h + 1);
        let mut glyph_pixels = Vec::with_capacity(w * h);
        for gy in 0..h {
            for gx in 0..w {
                glyph_pixels.push(pixels[(y0 + gy) * bmp_w + (x0 + gx)]);
            }
        }
        glyphs.push(pixels_to_glyph(&glyph_pixels, meta.width, meta.height));
    }

    let font = psf::PsfFont { meta, glyphs };
    let psf_data = psf::serialize(&font)?;

    let psf_path = bmp_path.with_extension("psf");
    std::fs::write(&psf_path, &psf_data)
        .map_err(|e| format!("cannot write {}: {}", psf_path.display(), e))?;

    println!("wrote {}", psf_path.display());
    Ok(())
}
```

**Step 4: Run tests**

```bash
cargo test tests::bmp_to_psf
```
Expected: PASS.

---

## Task 9: main.rs — `--new` mode + CLI wiring

**Files:**
- Modify: `src/main.rs`

No new tests needed — covered by existing module tests. Just wire everything up.

**Step 1: Implement `create_blank`**

```rust
fn create_blank(
    output: &std::path::Path,
    width: u32,
    height: u32,
    num_glyphs: u32,
    version: u8,
) -> Result<(), String> {
    if version == 1 && width != 8 {
        return Err("PSF1 only supports 8px-wide glyphs".to_string());
    }
    let meta = meta::FontMeta { version, width, height, num_glyphs, flags: 0 };

    let cols = 16usize;
    let w = width as usize;
    let h = height as usize;
    let n = num_glyphs as usize;
    let rows = (n + cols - 1) / cols;
    let bmp_w = 1 + cols * (w + 1);
    let bmp_h = 1 + rows * (h + 1);
    let red = [255u8, 0, 0];

    let mut pixels = vec![[0u8; 3]; bmp_w * bmp_h];
    for y in 0..bmp_h {
        for x in 0..bmp_w {
            if x % (w + 1) == 0 || y % (h + 1) == 0 {
                pixels[y * bmp_w + x] = red;
            }
        }
    }

    let bmp_path = output.with_extension("bmp");
    let json_path = output.with_extension("json");
    bmp::write_bmp(&bmp_path, bmp_w, bmp_h, &pixels)?;
    meta.save(&json_path)?;
    println!("created {} and {}", bmp_path.display(), json_path.display());
    Ok(())
}
```

**Step 2: Implement `main`**

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();

    // --new mode
    if args.iter().any(|a| a == "--new") {
        let width     = parse_flag(&args, "--width",  8u32);
        let height    = parse_flag(&args, "--height", 16u32);
        let num_glyphs = parse_flag(&args, "--glyphs", 256u32);
        let version   = parse_flag(&args, "--version", 2u8);
        let output = args.last().filter(|a| !a.starts_with('-'))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("font.bmp"));
        if let Err(e) = create_blank(&output, width, height, num_glyphs, version) {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Auto-detect by extension
    if args.len() < 2 {
        eprintln!("usage: psftool <font.psf|font.bmp>");
        eprintln!("       psftool --new [--width W] [--height H] [--glyphs N] [--version 1|2] output.bmp");
        std::process::exit(1);
    }

    let path = std::path::Path::new(&args[1]);
    let result = match path.extension().and_then(|e| e.to_str()) {
        Some("psf") => convert_psf_to_bmp(path),
        Some("bmp") => convert_bmp_to_psf(path),
        _ => Err(format!(
            "unknown file format '{}': expected .psf or .bmp",
            path.display()
        )),
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn parse_flag<T: std::str::FromStr>(args: &[String], flag: &str, default: T) -> T {
    args.windows(2)
        .find(|w| w[0] == flag)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}
```

**Step 3: Build and smoke test**

```bash
cargo build --release
echo "binary at target/release/psftool"
```

---

## Task 10: Integration test

**Files:**
- Create: `tests/integration.rs`

**Step 1: Write the test**

```rust
use std::process::Command;
use std::path::PathBuf;

fn psftool() -> PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop(); p.pop(); // remove deps/ if present
    if p.ends_with("deps") { p.pop(); }
    p.push("psftool");
    p
}

fn make_psf2_bytes(width: u32, height: u32, num_glyphs: u32) -> Vec<u8> {
    let bpg = height * ((width + 7) / 8);
    let mut d = vec![];
    d.extend_from_slice(&[0x72, 0xb5, 0x4a, 0x86]);
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&32u32.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&num_glyphs.to_le_bytes());
    d.extend_from_slice(&bpg.to_le_bytes());
    d.extend_from_slice(&height.to_le_bytes());
    d.extend_from_slice(&width.to_le_bytes());
    for i in 0..(num_glyphs * bpg) { d.push((i % 256) as u8); }
    d
}

#[test]
fn round_trip_psf2() {
    let tmp = std::env::temp_dir().join("psftool_integ");
    std::fs::create_dir_all(&tmp).unwrap();

    let psf_in  = tmp.join("font.psf");
    let bmp     = tmp.join("font.bmp");
    let json    = tmp.join("font.json");
    let psf_out = tmp.join("font_rt.psf");

    let original = make_psf2_bytes(8, 16, 32);
    std::fs::write(&psf_in, &original).unwrap();

    // PSF → BMP
    let status = Command::new(psftool()).arg(&psf_in).status().unwrap();
    assert!(status.success(), "psf→bmp failed");
    assert!(bmp.exists() && json.exists());

    // BMP → PSF (will write font.psf, overwriting original — copy first)
    std::fs::copy(&psf_in, &psf_out).unwrap();
    let status = Command::new(psftool()).arg(&bmp).status().unwrap();
    assert!(status.success(), "bmp→psf failed");

    // Compare glyph data (skip PSF2 header which may differ in flags reconstruction)
    let result = std::fs::read(&psf_in).unwrap(); // psftool wrote font.psf
    assert_eq!(&result[32..], &original[32..], "glyph data mismatch after round-trip");

    std::fs::remove_dir_all(&tmp).ok();
}
```

**Step 2: Run**

```bash
cargo test --test integration
```
Expected: PASS.

**Step 3: Run all tests**

```bash
cargo test
```
Expected: all PASS, no warnings about unused code.

---

## Done

The tool is complete. Optionally:
- `cargo build --release` for the optimized binary
- Test with a real PSF font: `psftool /usr/share/consolefonts/Lat15-Terminus16.psf`
