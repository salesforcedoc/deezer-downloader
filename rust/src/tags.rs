//! ID3v2.3 / ID3v1 tag writing.
//!
//! This mirrors the tags the Python app writes: text frames plus an embedded
//! JPEG cover, followed by a 128 byte ID3v1 tag after the audio.

pub struct TagData<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub album: &'a str,
    pub track_number: &'a str,
    pub disk_number: &'a str,
    pub year: &'a str,
    pub original_year: &'a str,
    pub date_ddmm: &'a str,
    pub label: &'a str,
    pub total_tracks: &'a str,
    pub duration_secs: u64,
    pub isrc: &'a str,
    pub file_size: u64,
    pub cover_jpeg: Option<&'a [u8]>,
}

fn frame(id: &[u8; 4], content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(10 + content.len());
    out.extend_from_slice(id);
    out.extend_from_slice(&(content.len() as u32).to_be_bytes());
    out.extend_from_slice(&[0, 0]); // flags
    out.extend_from_slice(content);
    out
}

fn text_frame(id: &[u8; 4], value: &str) -> Vec<u8> {
    if value.is_empty() {
        return Vec::new();
    }
    let mut content = Vec::with_capacity(value.len() + 1);
    content.push(0x03); // UTF-8
    content.extend_from_slice(value.as_bytes());
    frame(id, &content)
}

/// 28 bit syncsafe integer used by the ID3v2 header.
fn syncsafe(value: u32) -> u32 {
    ((value << 3) & 0x7f00_0000)
        | ((value << 2) & 0x007f_0000)
        | ((value << 1) & 0x0000_7f00)
        | (value & 0x7f)
}

pub fn build_id3v2(data: &TagData<'_>) -> Vec<u8> {
    let mut frames = Vec::new();

    let track = if data.total_tracks.is_empty() {
        data.track_number.to_string()
    } else {
        format!("{}/{}", data.track_number, data.total_tracks)
    };

    frames.extend(text_frame(b"TRCK", &track));
    frames.extend(text_frame(b"TLEN", &(data.duration_secs * 1000).to_string()));
    frames.extend(text_frame(b"TORY", data.original_year));
    frames.extend(text_frame(b"TYER", data.year));
    frames.extend(text_frame(b"TDAT", data.date_ddmm));
    frames.extend(text_frame(b"TPUB", data.label));
    frames.extend(text_frame(b"TSIZ", &data.file_size.to_string()));
    frames.extend(text_frame(b"TFLT", "MPG/3"));
    frames.extend(text_frame(b"TALB", data.album));
    frames.extend(text_frame(b"TPE1", data.artist));
    frames.extend(text_frame(b"TPE2", data.artist));
    frames.extend(text_frame(b"TPOS", data.disk_number));
    frames.extend(text_frame(b"TIT2", data.title));
    frames.extend(text_frame(b"TSRC", data.isrc));

    if let Some(cover) = data.cover_jpeg {
        let mut content = Vec::with_capacity(cover.len() + 20);
        content.push(0x00); // ISO-8859-1
        content.extend_from_slice(b"image/jpeg\0");
        content.push(0x03); // cover (front)
        content.push(0x00); // empty description
        content.extend_from_slice(cover);
        frames.extend(frame(b"APIC", &content));
    }

    let mut out = Vec::with_capacity(10 + frames.len());
    out.extend_from_slice(b"ID3");
    out.extend_from_slice(&[0x03, 0x00]); // v2.3, no flags
    out.push(0x00);
    out.extend_from_slice(&syncsafe(frames.len() as u32).to_be_bytes());
    out.extend_from_slice(&frames);
    out
}

fn copy_truncated(dst: &mut [u8], value: &str) {
    let bytes = value.as_bytes();
    let mut end = bytes.len().min(dst.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    dst[..end].copy_from_slice(&bytes[..end]);
}

pub fn build_id3v1(data: &TagData<'_>) -> [u8; 128] {
    let mut out = [0u8; 128];
    out[0..3].copy_from_slice(b"TAG");
    copy_truncated(&mut out[3..33], data.title);
    copy_truncated(&mut out[33..63], data.artist);
    copy_truncated(&mut out[63..93], data.album);
    copy_truncated(&mut out[93..97], data.original_year);
    // 28 byte comment stays zero, then byte 125 is the zero separator
    out[126] = data.track_number.parse().unwrap_or(0);
    out[127] = 255; // genre: unknown
    out
}

pub fn year_of(date: &str) -> &str {
    if date.len() >= 4 {
        &date[..4]
    } else {
        ""
    }
}

/// "2026-08-07" -> "0708"
pub fn ddmm_of(date: &str) -> String {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() >= 3 {
        format!("{}{}", parts[2], parts[1])
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample<'a>(cover: Option<&'a [u8]>) -> TagData<'a> {
        TagData {
            title: "Chasing",
            artist: "Southbound",
            album: "Chasing",
            track_number: "1",
            disk_number: "1",
            year: "2026",
            original_year: "2026",
            date_ddmm: "0708",
            label: "AAO Re",
            total_tracks: "1",
            duration_secs: 183,
            isrc: "QT6G22680866",
            file_size: 7343542,
            cover_jpeg: cover,
        }
    }

    #[test]
    fn writes_v2_header_and_frames() {
        let bytes = build_id3v2(&sample(Some(b"jpegdata")));
        assert_eq!(&bytes[0..3], b"ID3");
        assert_eq!(&bytes[3..5], &[0x03, 0x00]);
        let body = &bytes[10..];
        assert!(body.windows(4).any(|w| w == b"TIT2"));
        assert!(body.windows(4).any(|w| w == b"APIC"));
        assert!(body.windows(4).any(|w| w == b"TSRC"));
    }

    #[test]
    fn writes_v1_tag() {
        let v1 = build_id3v1(&sample(None));
        assert_eq!(&v1[0..3], b"TAG");
        assert_eq!(v1[126], 1);
        assert_eq!(v1[127], 255);
    }

    #[test]
    fn formats_dates() {
        assert_eq!(year_of("2026-08-07"), "2026");
        assert_eq!(ddmm_of("2026-08-07"), "0708");
        assert_eq!(ddmm_of(""), "");
    }
}
