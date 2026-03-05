use std::process::Command;
use std::path::PathBuf;

fn psftool_bin() -> PathBuf {
    // Locate the compiled binary in the Cargo output directory
    let mut p = std::env::current_exe().unwrap();
    p.pop(); // remove test binary name
    if p.ends_with("deps") {
        p.pop(); // remove deps/ dir
    }
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
    let psf_out = tmp.join("font.psf");

    let original = make_psf2_bytes(8, 16, 32);
    std::fs::write(&psf_in, &original).unwrap();

    // PSF → BMP
    let status = Command::new(psftool_bin())
        .arg(&psf_in)
        .status()
        .expect("failed to run psftool");
    assert!(status.success(), "psf→bmp conversion failed");
    assert!(bmp.exists(), "BMP file not created");
    assert!(json.exists(), "JSON sidecar not created");

    // BMP → PSF
    let status = Command::new(psftool_bin())
        .arg(&bmp)
        .status()
        .expect("failed to run psftool");
    assert!(status.success(), "bmp→psf conversion failed");

    // Compare glyph bytes (skip 32-byte PSF2 header which may differ slightly)
    let result = std::fs::read(&psf_out).unwrap();
    assert_eq!(
        &result[32..],
        &original[32..],
        "glyph data mismatch after round-trip"
    );

    std::fs::remove_dir_all(&tmp).ok();
}
