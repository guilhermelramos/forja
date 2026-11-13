//! Reads an image from the Windows clipboard and turns it into PNG bytes, so
//! the composer can attach a pasted screenshot (`chat.attachments.paste_images`).
//!
//! Only the clipboard read happens on the UI thread: it is a quick, in-memory
//! copy. Writing the PNG to disk runs in a background task.

/// PNG bytes of the clipboard image, when the clipboard holds an image and no text.
#[cfg(windows)]
pub fn clipboard_png() -> Option<Vec<u8>> {
    let dib = clipboard_dib()?;
    dib_to_png(&dib)
}

/// Other platforms have no clipboard image support in Forja yet.
#[cfg(not(windows))]
pub fn clipboard_png() -> Option<Vec<u8>> {
    None
}

#[cfg(windows)]
const CF_DIB: u32 = 8;
#[cfg(windows)]
const CF_UNICODETEXT: u32 = 13;

/// Raw `BITMAPINFO` bytes (DIB) copied from the clipboard.
#[cfg(windows)]
fn clipboard_dib() -> Option<Vec<u8>> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    // SAFETY: every handle is checked before use, and the clipboard is closed
    // on every path after a successful OpenClipboard.
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 {
            return None;
        }
        if IsClipboardFormatAvailable(CF_DIB) == 0 {
            return None;
        }
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let mut bytes = None;
        let handle = GetClipboardData(CF_DIB);
        if !handle.is_null() {
            let size = GlobalSize(handle);
            let pointer = GlobalLock(handle) as *const u8;
            if !pointer.is_null() && size > 0 {
                bytes = Some(std::slice::from_raw_parts(pointer, size).to_vec());
            }
            GlobalUnlock(handle);
        }
        CloseClipboard();
        bytes
    }
}

/// Wraps a DIB in a BMP file header and re-encodes it as PNG.
#[cfg(windows)]
fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>> {
    let bmp = dib_to_bmp(dib)?;
    let image = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp).ok()?;
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .ok()?;
    Some(out.into_inner())
}

/// Builds a BMP file from a DIB. The pixel data starts after the header, the
/// colour table and, for `BI_BITFIELDS` with a 40-byte header, the three masks.
#[cfg(any(windows, test))]
pub fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    const FILE_HEADER: usize = 14;
    let read_u32 = |at: usize| -> Option<u32> {
        dib.get(at..at + 4)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    let header_size = read_u32(0)? as usize;
    if header_size < 40 || header_size > dib.len() {
        return None;
    }
    let bits = u16::from_le_bytes([*dib.get(14)?, *dib.get(15)?]) as u32;
    let compression = read_u32(16)?;
    let colors_used = read_u32(32)? as usize;
    let palette_entries = if bits <= 8 {
        if colors_used == 0 {
            1usize << bits
        } else {
            colors_used
        }
    } else {
        colors_used
    };
    let masks = if compression == 3 && header_size == 40 { 12 } else { 0 };
    let pixel_offset = FILE_HEADER + header_size + masks + palette_entries * 4;
    if pixel_offset > FILE_HEADER + dib.len() {
        return None;
    }
    let total = FILE_HEADER + dib.len();
    let mut bmp = Vec::with_capacity(total);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(total as u32).to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(bmp)
}

#[cfg(test)]
mod tests {
    use super::dib_to_bmp;

    fn dib_header(width: i32, height: i32, bits: u16) -> Vec<u8> {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&width.to_le_bytes());
        dib.extend_from_slice(&height.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&bits.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&0i32.to_le_bytes());
        dib.extend_from_slice(&0i32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib
    }

    #[test]
    fn bmp_pixel_offset_follows_header_for_truecolor() {
        let mut dib = dib_header(1, 1, 24);
        dib.extend_from_slice(&[0, 0, 255, 0]);
        let bmp = dib_to_bmp(&dib).expect("valid dib");
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(u32::from_le_bytes([bmp[10], bmp[11], bmp[12], bmp[13]]), 54);
        assert_eq!(bmp.len(), 14 + dib.len());
    }

    #[test]
    fn bmp_skips_palette_for_indexed_images() {
        let mut dib = dib_header(1, 1, 8);
        dib.extend(std::iter::repeat_n(0, 256 * 4));
        dib.extend_from_slice(&[0, 0, 0, 0]);
        let bmp = dib_to_bmp(&dib).expect("valid dib");
        assert_eq!(
            u32::from_le_bytes([bmp[10], bmp[11], bmp[12], bmp[13]]) as usize,
            14 + 40 + 256 * 4
        );
    }

    #[test]
    fn truncated_dib_is_rejected() {
        assert!(dib_to_bmp(&[1, 2, 3]).is_none());
        assert!(dib_to_bmp(&[]).is_none());
    }
}
