// BMP read/write

use std::path::Path;

pub fn write_bmp(path: &Path, width: usize, height: usize, pixels: &[[u8; 3]]) -> Result<(), String> {
    if pixels.len() != width * height {
        return Err(format!(
            "pixel buffer length {} does not match {}x{} = {}",
            pixels.len(), width, height, width * height
        ));
    }
    let stride = (width * 3 + 3) & !3;
    let pixel_data_size = stride * height;
    let file_size = 54 + pixel_data_size;

    let mut buf: Vec<u8> = Vec::with_capacity(file_size);

    // File header
    buf.extend_from_slice(b"BM");
    buf.extend_from_slice(&(file_size as u32).to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&54u32.to_le_bytes());

    // DIB header (BITMAPINFOHEADER)
    buf.extend_from_slice(&40u32.to_le_bytes());
    buf.extend_from_slice(&(width as i32).to_le_bytes());
    buf.extend_from_slice(&(-(height as i32)).to_le_bytes()); // negative = top-down
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&24u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&2835i32.to_le_bytes());
    buf.extend_from_slice(&2835i32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());

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
    if width <= 0 {
        return Err(format!("invalid BMP width: {}", width));
    }
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
