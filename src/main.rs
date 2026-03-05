mod meta;
mod bmp;
mod psf;

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

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--new") {
        let width      = parse_flag(&args, "--width",   8u32);
        let height     = parse_flag(&args, "--height",  16u32);
        let num_glyphs = parse_flag(&args, "--glyphs",  256u32);
        let version    = parse_flag(&args, "--version", 2u8);
        let output = args[1..].iter()
            .filter(|a| !a.starts_with('-'))
            .last()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("font.bmp"));
        if let Err(e) = create_blank(&output, width, height, num_glyphs, version) {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
        return;
    }

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

    // Fill separator pixels red
    for y in 0..bmp_h {
        for x in 0..bmp_w {
            if x % (w + 1) == 0 || y % (h + 1) == 0 {
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
    let rows = (n + cols - 1) / cols;

    let expected_w = 1 + cols * (w + 1);
    let expected_h = 1 + rows * (h + 1);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_to_pixels_full() {
        // 8x2 glyph: first row all-on (0xFF), second row all-off (0x00)
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

    #[test]
    fn psf_to_bmp_creates_files() {
        let mut psf_data = vec![];
        psf_data.extend_from_slice(&[0x72, 0xb5, 0x4a, 0x86]);
        psf_data.extend_from_slice(&0u32.to_le_bytes());
        psf_data.extend_from_slice(&32u32.to_le_bytes());
        psf_data.extend_from_slice(&0u32.to_le_bytes());
        psf_data.extend_from_slice(&2u32.to_le_bytes());  // 2 glyphs
        psf_data.extend_from_slice(&2u32.to_le_bytes());  // 2 bytes per glyph
        psf_data.extend_from_slice(&2u32.to_le_bytes());  // height 2
        psf_data.extend_from_slice(&8u32.to_le_bytes());  // width 8
        psf_data.extend_from_slice(&[0xFF, 0x00]);
        psf_data.extend_from_slice(&[0x00, 0xFF]);

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

    #[test]
    fn bmp_to_psf_round_trip() {
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
        let psf_in   = tmp_dir.join("rt_font.psf");
        let bmp_path = tmp_dir.join("rt_font.bmp");
        let psf_out  = tmp_dir.join("rt_font.psf"); // psftool writes back to same stem

        std::fs::write(&psf_in, &psf_data).unwrap();
        convert_psf_to_bmp(&psf_in).unwrap();
        // bmp_path and rt_font.json now exist
        convert_bmp_to_psf(&bmp_path).unwrap();

        let result = std::fs::read(&psf_out).unwrap();
        // Compare glyph bytes only (skip PSF2 32-byte header)
        assert_eq!(&result[32..], &psf_data[32..], "glyph data mismatch after round-trip");

        std::fs::remove_file(&psf_in).ok();
        std::fs::remove_file(&bmp_path).ok();
        std::fs::remove_file(&tmp_dir.join("rt_font.json")).ok();
    }
}
