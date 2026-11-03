//! Bounded Kitty graphics subset: direct RGB/RGBA/PNG, chunks, stored placements
//! and deletion. File/shared-memory transport and all unlisted controls fail.
//!
//! Accepted controls: a, t, f, i, p, s, v, c, r, m, q, d. Formats are
//! 24-bit RGB, 32-bit RGBA and 100 (PNG). Only direct transport is accepted;
//! cropping, compression, z-order, animation and virtual placements are rejected.
//! One transfer can be assembled at a time, with at most 16 MiB of base64.
//! Images are limited to 4096 per axis and 64 MiB of retained RGBA; image and
//! placement counts are each capped at 4096. q=1 suppresses success and q=2
//! suppresses all replies. Intermediate chunks never receive success replies.
//! Deletion supports all (a/A) or image ID (i/I), optionally a placement ID;
//! selected images without remaining placements release their decoded storage.
//! Without c/r, natural size uses the default 10x20 cell metrics. Explicit c/r
//! is recommended since this GPU-independent layer has no window/DPI metrics.
use crate::ImagePlacement;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{collections::BTreeMap, io::Cursor, sync::Arc};

const TRANSFER_LIMIT: usize = 16 * 1024 * 1024;
const PIXEL_LIMIT: usize = 64 * 1024 * 1024;
const OBJECT_LIMIT: usize = 4096;
type Controls = BTreeMap<u8, String>;
#[derive(Clone)]
struct Image {
    width: u32,
    height: u32,
    rgba: Arc<Vec<u8>>,
}
struct Transfer {
    controls: Controls,
    encoded: Vec<u8>,
    cursor: (usize, usize),
}
#[derive(Default)]
pub(crate) struct Kitty {
    images: BTreeMap<u32, Image>,
    placements: Vec<ImagePlacement>,
    pending: Option<Transfer>,
    next_id: u32,
}
fn number(c: &Controls, key: u8, default: u32) -> Result<u32, &'static str> {
    c.get(&key).map_or(Ok(default), |v| {
        v.parse().map_err(|_| "EINVAL:invalid number")
    })
}
fn byte(c: &Controls, key: u8, default: u8) -> Result<u8, &'static str> {
    match c.get(&key) {
        None => Ok(default),
        Some(v) if v.len() == 1 => Ok(v.as_bytes()[0]),
        _ => Err("EINVAL:invalid control"),
    }
}
fn controls(header: &[u8]) -> Result<Controls, &'static str> {
    let mut c = Controls::new();
    if header.is_empty() {
        return Ok(c);
    }
    for field in header.split(|b| *b == b',') {
        if field.len() < 3 || field[1] != b'=' || c.contains_key(&field[0]) {
            return Err("EINVAL:invalid header");
        }
        let value = std::str::from_utf8(&field[2..]).map_err(|_| "EINVAL:invalid header")?;
        c.insert(field[0], value.into());
    }
    Ok(c)
}
impl Kitty {
    pub fn dispatch(
        &mut self,
        payload: &[u8],
        cursor: (usize, usize),
        viewport: (usize, usize),
    ) -> Option<Vec<u8>> {
        let payload = payload.strip_prefix(b"G")?;
        let (header, data) = match payload.iter().position(|b| *b == b';') {
            Some(i) => (&payload[..i], &payload[i + 1..]),
            None => (payload, &[][..]),
        };
        // A small header also bounds allocations for malformed control lists.
        let parsed = if header.len() > 4096 {
            Err("E2BIG:header too large")
        } else {
            controls(header)
        };
        let mut c = match parsed {
            Ok(c) => c,
            Err(e) => {
                self.pending = None;
                return Some(format!("\x1b_G;{e}\x1b\\").into_bytes());
            }
        };
        let mut reply_c = c.clone();
        if let Some(p) = &self.pending {
            reply_c = p.controls.clone();
            for (k, v) in &c {
                reply_c.insert(*k, v.clone());
            }
        }
        let result = self.process(&mut c, data, cursor, viewport);
        if result.is_err() {
            self.pending = None;
        }
        let q = number(&reply_c, b'q', 0).unwrap_or(0);
        match result {
            Ok(None) => None, // Intermediate chunks never receive a reply.
            Ok(Some(id)) if q == 0 => Some(reply(&reply_c, id, "OK")),
            Err(e) if q < 2 => Some(reply(&reply_c, number(&reply_c, b'i', 0).unwrap_or(0), e)),
            _ => None,
        }
    }
    fn process(
        &mut self,
        c: &mut Controls,
        data: &[u8],
        cursor: (usize, usize),
        viewport: (usize, usize),
    ) -> Result<Option<u32>, &'static str> {
        for key in c.keys() {
            if !b"aftipsvcrmqd".contains(key) {
                return Err("ENOTSUP:unsupported control");
            }
        }
        let q = number(c, b'q', 0)?;
        if q > 2 {
            return Err("EINVAL:invalid quiet mode");
        }
        let more = number(c, b'm', 0)?;
        if more > 1 {
            return Err("EINVAL:invalid chunk flag");
        }
        let mut origin = cursor;
        let encoded;
        if let Some(mut pending) = self.pending.take() {
            for (key, value) in c.iter() {
                if !matches!(*key, b'm' | b'q')
                    && pending.controls.get(key).is_none_or(|old| old != value)
                {
                    return Err("EINVAL:chunk controls changed");
                }
            }
            if pending.encoded.len().saturating_add(data.len()) > TRANSFER_LIMIT {
                return Err("E2BIG:transfer too large");
            }
            pending.encoded.extend_from_slice(data);
            if more == 1 {
                self.pending = Some(pending);
                return Ok(None);
            }
            origin = pending.cursor;
            let quiet = c.get(&b'q').cloned();
            *c = pending.controls;
            if let Some(quiet) = quiet {
                c.insert(b'q', quiet);
            }
            encoded = pending.encoded;
        } else {
            if data.len() > TRANSFER_LIMIT {
                return Err("E2BIG:transfer too large");
            }
            if more == 1 {
                if !matches!(byte(c, b'a', b't')?, b't' | b'T') {
                    return Err("EINVAL:only transmissions may be chunked");
                }
                self.validate_transfer(c)?;
                self.pending = Some(Transfer {
                    controls: c.clone(),
                    encoded: data.to_vec(),
                    cursor,
                });
                return Ok(None);
            }
            encoded = data.to_vec();
        }
        let action = byte(c, b'a', b't')?;
        let mut id = number(c, b'i', 0)?;
        let pid = number(c, b'p', 0)?;
        match action {
            b't' | b'T' => {
                self.validate_transfer(c)?;
                if c.contains_key(&b'd') {
                    return Err("EINVAL:deletion control on transfer");
                }
                let bytes = STANDARD
                    .decode(&encoded)
                    .map_err(|_| "EINVAL:invalid base64")?;
                let image = decode(c, &bytes)?;
                let old = self.images.get(&id).map_or(0, |v| v.rgba.len());
                let retained: usize = self.images.values().map(|v| v.rgba.len()).sum();
                if retained - old + image.rgba.len() > PIXEL_LIMIT {
                    return Err("ENOSPC:pixel budget exceeded");
                }
                if id == 0 {
                    // Generated IDs are never zero and never overwrite a live image.
                    loop {
                        self.next_id = self.next_id.wrapping_add(1).max(1);
                        if !self.images.contains_key(&self.next_id) {
                            break;
                        }
                    }
                    id = self.next_id;
                }
                if !self.images.contains_key(&id) && self.images.len() >= OBJECT_LIMIT {
                    return Err("ENOSPC:too many images");
                }
                if action == b'T' {
                    self.place(id, pid, &image, c, origin, viewport)?;
                }
                // Retransmission refreshes existing placements without retaining old buffers.
                for p in &mut self.placements {
                    if p.image_id == id {
                        p.rgba = image.rgba.clone();
                        p.width = image.width;
                        p.height = image.height;
                    }
                }
                self.images.insert(id, image);
            }
            b'p' => {
                if !encoded.is_empty() || more != 0 || c.keys().any(|k| !b"aipcrq".contains(k)) {
                    return Err("ENOTSUP:unsupported placement control");
                }
                let image = self
                    .images
                    .get(&id)
                    .ok_or("ENOENT:image not found")?
                    .clone();
                self.place(id, pid, &image, c, origin, viewport)?;
            }
            b'd' => {
                if !encoded.is_empty() || more != 0 || c.keys().any(|k| !b"adipq".contains(k)) {
                    return Err("ENOTSUP:unsupported deletion control");
                }
                let mode = byte(c, b'd', b'a')?;
                match mode {
                    b'a' | b'A' => {
                        self.placements.clear();
                        self.images.clear();
                    }
                    b'i' | b'I' => {
                        self.placements
                            .retain(|p| p.image_id != id || (pid != 0 && p.placement_id != pid));
                        if !self.placements.iter().any(|p| p.image_id == id) {
                            self.images.remove(&id);
                        }
                    }
                    _ => return Err("ENOTSUP:unsupported deletion mode"),
                }
            }
            _ => return Err("ENOTSUP:unsupported action"),
        }
        Ok(Some(id))
    }
    fn validate_transfer(&self, c: &Controls) -> Result<(), &'static str> {
        if byte(c, b't', b'd')? != b'd' {
            return Err("ENOTSUP:only direct transport supported");
        }
        if !matches!(number(c, b'f', 32)?, 24 | 32 | 100) {
            return Err("ENOTSUP:unsupported pixel format");
        }
        Ok(())
    }
    fn place(
        &mut self,
        id: u32,
        pid: u32,
        image: &Image,
        c: &Controls,
        cursor: (usize, usize),
        viewport: (usize, usize),
    ) -> Result<(), &'static str> {
        let cols = number(c, b'c', image.width.div_ceil(10))? as usize;
        let rows = number(c, b'r', image.height.div_ceil(20))? as usize;
        if cols == 0 || rows == 0 || cols > 65535 || rows > 65535 {
            return Err("EINVAL:invalid placement dimensions");
        }
        let existing = if pid != 0 {
            self.placements
                .iter()
                .position(|p| p.image_id == id && p.placement_id == pid)
        } else {
            None
        };
        if existing.is_none() && self.placements.len() >= OBJECT_LIMIT {
            return Err("ENOSPC:too many placements");
        }
        let p = ImagePlacement {
            image_id: id,
            placement_id: pid,
            x: cursor.1,
            y: cursor.0,
            cols,
            rows,
            width: image.width,
            height: image.height,
            rgba: image.rgba.clone(),
            clip_x: 0,
            clip_y: 0,
            clip_cols: viewport.1,
            clip_rows: viewport.0,
        };
        if let Some(i) = existing {
            self.placements[i] = p;
        } else {
            self.placements.push(p);
        }
        Ok(())
    }
    pub fn snapshot(&self, rows: usize, cols: usize) -> Vec<ImagePlacement> {
        self.placements
            .iter()
            .map(|p| {
                let mut p = p.clone();
                p.clip_rows = rows;
                p.clip_cols = cols;
                p
            })
            .collect()
    }
}
fn reply(c: &Controls, id: u32, status: &str) -> Vec<u8> {
    let pid = number(c, b'p', 0).unwrap_or(0);
    let placement = if pid != 0 {
        format!(",p={pid}")
    } else {
        String::new()
    };
    format!("\x1b_Gi={id}{placement};{status}\x1b\\").into_bytes()
}
fn dimensions(width: u32, height: u32) -> Result<usize, &'static str> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err("E2BIG:invalid image dimensions");
    }
    Ok(width as usize * height as usize * 4)
}
fn decode(c: &Controls, bytes: &[u8]) -> Result<Image, &'static str> {
    let format = number(c, b'f', 32)?;
    let (width, height, rgba) = if format == 100 {
        let mut decoder =
            png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: PIXEL_LIMIT });
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|_| "EINVAL:invalid PNG")?;
        let width = reader.info().width;
        let height = reader.info().height;
        let size = dimensions(width, height)?;
        let output = reader.output_buffer_size().ok_or("E2BIG:PNG too large")?;
        if output > size {
            return Err("E2BIG:PNG too large");
        }
        let mut pixels = vec![0; output];
        let info = reader
            .next_frame(&mut pixels)
            .map_err(|_| "EINVAL:invalid PNG frame")?;
        let mut rgba = Vec::with_capacity(size);
        let samples = &pixels[..info.buffer_size()];
        match info.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(samples),
            png::ColorType::Rgb => {
                for p in samples.chunks_exact(3) {
                    rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
                }
            }
            png::ColorType::Grayscale => {
                for p in samples {
                    rgba.extend_from_slice(&[*p, *p, *p, 255]);
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for p in samples.chunks_exact(2) {
                    rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]]);
                }
            }
            _ => return Err("ENOTSUP:unsupported PNG colour type"),
        }
        if rgba.len() != size {
            return Err("EINVAL:invalid PNG dimensions");
        }
        (width, height, rgba)
    } else {
        let width = number(c, b's', 0)?;
        let height = number(c, b'v', 0)?;
        let size = dimensions(width, height)?;
        let channels = if format == 24 { 3 } else { 4 };
        if bytes.len() != size / 4 * channels {
            return Err("EINVAL:pixel length mismatch");
        }
        let rgba = if channels == 4 {
            bytes.to_vec()
        } else {
            let mut rgba = Vec::with_capacity(size);
            for p in bytes.chunks_exact(3) {
                rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
            rgba
        };
        (width, height, rgba)
    };
    Ok(Image {
        width,
        height,
        rgba: Arc::new(rgba),
    })
}
