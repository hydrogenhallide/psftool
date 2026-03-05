use crate::meta::FontMeta;

const PSF1_MAGIC: [u8; 2] = [0x36, 0x04];
const PSF2_MAGIC: [u8; 4] = [0x72, 0xb5, 0x4a, 0x86];

pub struct PsfFont {
    pub meta: FontMeta,
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
    if charsize == 0 {
        return Err("PSF1 charsize is 0, invalid font".to_string());
    }
    let num_glyphs: usize = if mode & 0x01 != 0 { 512 } else { 256 };
    let expected = num_glyphs.checked_mul(charsize)
        .and_then(|n| n.checked_add(4))
        .ok_or_else(|| "PSF1 glyph count overflow".to_string())?;
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
    if headersize < 32 {
        return Err(format!("PSF2 headersize {} is less than minimum 32", headersize));
    }
    let flags       = read_u32(data, 12)?;
    let num_glyphs  = read_u32(data, 16)? as usize;
    let bytes_per_g = read_u32(data, 20)? as usize;
    let height      = read_u32(data, 24)?;
    let width       = read_u32(data, 28)?;

    let expected_bytes_per_g = (height as usize)
        .checked_mul(((width as usize) + 7) / 8)
        .ok_or_else(|| "PSF2 glyph dimensions overflow".to_string())?;
    if bytes_per_g < expected_bytes_per_g {
        return Err(format!(
            "PSF2 bytes_per_glyph {} is less than width/height imply ({})",
            bytes_per_g, expected_bytes_per_g
        ));
    }

    let expected = num_glyphs.checked_mul(bytes_per_g)
        .and_then(|n| n.checked_add(headersize))
        .ok_or_else(|| "PSF2 glyph count overflow".to_string())?;
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
    for (i, glyph) in font.glyphs.iter().enumerate() {
        if glyph.len() != charsize {
            return Err(format!(
                "PSF1 glyph {} size mismatch: expected {}, got {}",
                i, charsize, glyph.len()
            ));
        }
        out.extend_from_slice(glyph);
    }
    Ok(out)
}

fn serialize_psf2(font: &PsfFont) -> Result<Vec<u8>, String> {
    let bytes_per_glyph = (font.meta.height * ((font.meta.width + 7) / 8)) as usize;
    let mut out = Vec::new();
    out.extend_from_slice(&PSF2_MAGIC);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    out.extend_from_slice(&font.meta.flags.to_le_bytes());
    out.extend_from_slice(&font.meta.num_glyphs.to_le_bytes());
    out.extend_from_slice(&(bytes_per_glyph as u32).to_le_bytes());
    out.extend_from_slice(&font.meta.height.to_le_bytes());
    out.extend_from_slice(&font.meta.width.to_le_bytes());
    for glyph in &font.glyphs {
        if glyph.len() != bytes_per_glyph {
            return Err(format!(
                "glyph size mismatch: expected {}, got {}",
                bytes_per_glyph, glyph.len()
            ));
        }
        out.extend_from_slice(glyph);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_psf2(width: u32, height: u32, num_glyphs: u32, glyph_byte: u8) -> Vec<u8> {
        let bytes_per_glyph = height * ((width + 7) / 8);
        let mut data = vec![];
        data.extend_from_slice(&[0x72, 0xb5, 0x4a, 0x86]);
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&32u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
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
        assert_eq!(font.glyphs[0].len(), 16);
        assert_eq!(font.glyphs[0][0], 0xAA);
    }

    #[test]
    fn parse_psf1_basic() {
        let mut data = vec![0x36, 0x04];
        data.push(0x00); // mode: 256 glyphs
        data.push(16);   // charsize = height = 16
        for _ in 0..(256 * 16) { data.push(0xFF); }
        let font = parse(&data).unwrap();
        assert_eq!(font.meta.version, 1);
        assert_eq!(font.meta.width, 8);
        assert_eq!(font.meta.height, 16);
        assert_eq!(font.meta.num_glyphs, 256);
        assert_eq!(font.glyphs[0][0], 0xFF);
    }

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
}
