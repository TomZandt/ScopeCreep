//! Reading Resolve's frames: float EXR (the probe's dump and Resolve's reference) and 16-bit TIFF (reference).
//! This tool is a separate process, so Rust EXR/TIFF crates are fine (OpenEXR must never be linked into the
//! plugin Resolve loads).

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// RGB float image, row-major, top row first.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<[f32; 3]>,
}

impl Image {
    pub fn pixel(&self, x: usize, y: usize) -> [f32; 3] {
        self.rgb[y * self.width + x]
    }
}

pub fn read_exr(path: &Path) -> Result<Image, String> {
    use exr::prelude::{read_first_rgba_layer_from_file, Vec2};
    let img = read_first_rgba_layer_from_file(
        path,
        |res: Vec2<usize>, _| Image { width: res.width(), height: res.height(), rgb: vec![[0.0; 3]; res.width() * res.height()] },
        |img: &mut Image, pos, (r, g, b, _a): (f32, f32, f32, f32)| {
            img.rgb[pos.y() * img.width + pos.x()] = [r, g, b];
        },
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(img.layer_data.channel_data.pixels)
}

/// 16-bit RGB TIFF as values in 0..1 (code / 65535).
pub fn read_tiff16(path: &Path) -> Result<Image, String> {
    use tiff::decoder::{Decoder, DecodingResult};
    let f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut dec = Decoder::new(BufReader::new(f)).map_err(|e| format!("{}: {e}", path.display()))?;
    let (w, h) = dec.dimensions().map_err(|e| e.to_string())?;
    let ct = dec.colortype().map_err(|e| e.to_string())?;
    let n = match ct {
        tiff::ColorType::RGB(16) => 3,
        tiff::ColorType::RGBA(16) => 4,
        other => return Err(format!("{}: unsupported TIFF colour type {other:?}", path.display())),
    };
    let data = match dec.read_image().map_err(|e| e.to_string())? {
        DecodingResult::U16(v) => v,
        _ => return Err(format!("{}: not 16-bit", path.display())),
    };
    let (w, h) = (w as usize, h as usize);
    let rgb = (0..w * h)
        .map(|i| [data[i * n] as f32 / 65535.0, data[i * n + 1] as f32 / 65535.0, data[i * n + 2] as f32 / 65535.0])
        .collect();
    Ok(Image { width: w, height: h, rgb })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("scopecreep-accuracy-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn exr_round_trip_is_bit_exact_for_floats_including_out_of_range() {
        use exr::prelude::*;
        let vals = [-0.25f32, 0.0, 0.18, 1.0, 10.0, 1234.5];
        let p = tmp("rt.exr");
        write_rgba_file(&p, 3, 2, |x, y| {
            let i = (y * 3 + x) % vals.len();
            (vals[i], vals[(i + 1) % vals.len()], vals[(i + 2) % vals.len()], 1.0f32)
        })
        .unwrap();
        let img = read_exr(&p).unwrap();
        assert_eq!((img.width, img.height), (3, 2));
        for y in 0..2 {
            for x in 0..3 {
                let i = (y * 3 + x) % vals.len();
                assert_eq!(img.pixel(x, y), [vals[i], vals[(i + 1) % 6], vals[(i + 2) % 6]]);
            }
        }
    }

    #[test]
    fn tiff16_reads_codes_as_fractions_of_65535() {
        use tiff::encoder::{TiffEncoder, colortype::RGB16};
        let p = tmp("rt.tif");
        let data: Vec<u16> = vec![0, 65535, 32768, 1, 2, 3];
        let mut f = File::create(&p).unwrap();
        TiffEncoder::new(&mut f).unwrap().write_image::<RGB16>(2, 1, &data).unwrap();
        drop(f);
        let img = read_tiff16(&p).unwrap();
        assert_eq!(img.pixel(0, 0), [0.0, 1.0, 32768.0 / 65535.0]);
        assert_eq!(img.pixel(1, 0), [1.0 / 65535.0, 2.0 / 65535.0, 3.0 / 65535.0]);
    }
}
