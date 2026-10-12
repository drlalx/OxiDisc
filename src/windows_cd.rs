use crate::discid::CdToc;
use crate::wav::CDDA_BYTES_PER_SECTOR;
use std::ffi::c_void;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: u32,
        dwShareMode: u32,
        lpSecurityAttributes: *const c_void,
        dwCreationDisposition: u32,
        dwFlagsAndAttributes: u32,
        hTemplateFile: isize,
    ) -> isize;

    fn CloseHandle(hObject: isize) -> i32;

    fn GetLastError() -> u32;

    fn DeviceIoControl(
        hDevice: isize,
        dwIoControlCode: u32,
        lpInBuffer: *const c_void,
        nInBufferSize: u32,
        lpOutBuffer: *mut c_void,
        nOutBufferSize: u32,
        lpBytesReturned: *mut u32,
        lpOverlapped: *mut c_void,
    ) -> i32;

    fn GetLogicalDrives() -> u32;

    fn GetDriveTypeW(lpRootPathName: *const u16) -> u32;
}

#[cfg(windows)]
#[link(name = "winmm")]
unsafe extern "system" {
    fn mciSendStringW(
        lpszCommand: *const u16,
        lpszReturnString: *mut u16,
        cchReturn: u32,
        hwndCallback: isize,
    ) -> u32;
}

// Windows Win32 constants
pub const INVALID_HANDLE_VALUE: isize = -1;
pub const GENERIC_READ: u32 = 0x80000000;
pub const FILE_SHARE_READ: u32 = 1;
pub const FILE_SHARE_WRITE: u32 = 2;
pub const OPEN_EXISTING: u32 = 3;

pub const IOCTL_CDROM_READ_TOC: u32 = 0x00024000;
pub const IOCTL_CDROM_RAW_READ: u32 = 0x0002403E;
pub const IOCTL_STORAGE_EJECT_MEDIA: u32 = 0x002D4808;
pub const IOCTL_STORAGE_LOAD_MEDIA: u32 = 0x002D480C;

/// Structure representing a Windows CDROM TOC Track Data entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WinTrackData {
    pub reserved: u8,
    pub control: u8,
    pub track_number: u8,
    pub reserved1: u8,
    pub address: [u8; 4], // [0] = reserved, [1] = Minute, [2] = Second, [3] = Frame
}

/// Structure representing the Windows CDROM TOC layout (IOCTL_CDROM_READ_TOC)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WinCdromToc {
    pub length: [u8; 2],
    pub first_track: u8,
    pub last_track: u8,
    pub track_data: [WinTrackData; 100],
}

/// Structure for RAW sector read requests (IOCTL_CDROM_RAW_READ)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WinRawReadInfo {
    pub disk_offset: i64,
    pub sector_count: u32,
    pub track_mode: u32, // CDDA = 2
}

pub const TRACK_MODE_CDDA: u32 = 2;
pub const MAX_SECTORS_PER_READ: u32 = 20; // 20 * 2352 = 47,040 bytes per buffer

/// Converts MSF (Minute, Second, Frame) into a sector count / frame offset.
pub fn msf_to_sectors(minute: u8, second: u8, frame: u8) -> u32 {
    let m = minute as u32;
    let s = second as u32;
    let f = frame as u32;
    (m * 60 + s) * 75 + f
}

/// Normalizes a Windows device string into the canonical Win32 device format `\\.\X:`.
pub fn normalize_device_path(dev: &str) -> String {
    let trimmed = dev.trim();
    if trimmed.is_empty() {
        return "\\\\.\\D:".to_string();
    }

    // Already in \\.\X: format
    if trimmed.starts_with(r"\\.\") {
        return trimmed.to_string();
    }

    // Handles "D:", "D:\", "d", "D"
    let mut drive_char = 'D';
    for ch in trimmed.chars() {
        if ch.is_ascii_alphabetic() {
            drive_char = ch.to_ascii_uppercase();
            break;
        }
    }

    format!(r"\\.\{}:", drive_char)
}

/// Enumerate all optical (CD/DVD/BD) drives available on the Windows system.
pub fn detect_windows_cd_drives() -> Vec<String> {
    let mut drives = Vec::new();

    #[cfg(windows)]
    {
        let mask = unsafe { GetLogicalDrives() };
        for i in 0..26 {
            if (mask & (1 << i)) != 0 {
                let drive_letter = (b'A' + i) as char;
                let root_path: Vec<u16> = format!("{}:\\\0", drive_letter).encode_utf16().collect();
                // DRIVE_CDROM is 5
                let drive_type = unsafe { GetDriveTypeW(root_path.as_ptr()) };
                if drive_type == 5 {
                    drives.push(format!("{}:", drive_letter));
                }
            }
        }
    }

    if drives.is_empty() {
        drives.push("D:".to_string());
    }

    drives
}

/// Extracts the drive root letter like "D:" from paths like "D:", "D:\\", or "\\\\.\\D:"
pub fn extract_drive_root(device_path: &str) -> String {
    let trimmed = device_path.trim();
    if let Some(rest) = trimmed.strip_prefix(r"\\.\") {
        if rest.len() >= 2 && rest.chars().nth(1) == Some(':') {
            return rest[0..2].to_uppercase();
        }
    }
    if trimmed.len() >= 2 && trimmed.chars().nth(1) == Some(':') {
        return trimmed[0..2].to_uppercase();
    }
    "D:".to_string()
}

/// Reads the CD Table of Contents (TOC) using Win32 DeviceIoControl with spin-up retry and MCI fallback.
pub fn read_toc_windows(device_path: &str) -> Result<CdToc, String> {
    if crate::mock_cd::is_mock_device(device_path) {
        return crate::mock_cd::get_mock_toc(device_path);
    }

    #[cfg(windows)]
    {
        let canonical_path = normalize_device_path(device_path);
        let wide_path: Vec<u16> = canonical_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut last_err = 0;
        let mut win_toc: WinCdromToc = unsafe { std::mem::zeroed() };
        let mut got_toc = false;

        // Try IOCTL_CDROM_READ_TOC with spin-up retry loop (up to 5 attempts)
        for attempt in 0..5 {
            let handle = unsafe {
                CreateFileW(
                    wide_path.as_ptr(),
                    GENERIC_READ,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    0,
                )
            };

            if handle == INVALID_HANDLE_VALUE {
                last_err = unsafe { GetLastError() };
                if (last_err == 21 || last_err == 1117) && attempt < 4 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    continue;
                }
                break;
            }

            let mut bytes_returned: u32 = 0;
            let success = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_CDROM_READ_TOC,
                    std::ptr::null(),
                    0,
                    &mut win_toc as *mut _ as *mut _,
                    std::mem::size_of::<WinCdromToc>() as u32,
                    &mut bytes_returned,
                    std::ptr::null_mut(),
                )
            };

            unsafe { CloseHandle(handle) };

            if success != 0 {
                got_toc = true;
                break;
            } else {
                last_err = unsafe { GetLastError() };
                if (last_err == 21 || last_err == 1117) && attempt < 4 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    continue;
                }
            }
        }

        if got_toc {
            if let Ok(toc) = parse_win_toc(&win_toc) {
                return Ok(toc);
            }
        }

        // Secondary fallback: MCI Table of Contents reader
        if let Ok(mci_toc) = read_toc_windows_mci(device_path) {
            return Ok(mci_toc);
        }

        Err(format!(
            "Failed to read TOC from drive '{canonical_path}' (Win32 error: {last_err})"
        ))
    }

    #[cfg(not(windows))]
    {
        let _ = device_path;
        Err("Win32 TOC reader is only available on Windows".to_string())
    }
}

/// Fallback TOC reader using Windows MCI commands.
#[cfg(windows)]
pub fn read_toc_windows_mci(device_path: &str) -> Result<CdToc, String> {
    let drive = extract_drive_root(device_path);
    let alias = "oxidisc_toc_read";

    let open_cmd: Vec<u16> = format!("open {drive} type cdaudio alias {alias}\0")
        .encode_utf16()
        .collect();
    let set_msf_cmd: Vec<u16> = format!("set {alias} time format msf\0")
        .encode_utf16()
        .collect();
    let status_tracks_cmd: Vec<u16> = format!("status {alias} number of tracks\0")
        .encode_utf16()
        .collect();
    let close_cmd: Vec<u16> = format!("close {alias}\0").encode_utf16().collect();

    unsafe {
        mciSendStringW(close_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);

        let res_open = mciSendStringW(open_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);
        if res_open != 0 {
            return Err(format!(
                "MCI open drive {drive} failed with code {res_open}"
            ));
        }

        mciSendStringW(set_msf_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);

        let mut buf = [0u16; 128];
        let res_tracks = mciSendStringW(status_tracks_cmd.as_ptr(), buf.as_mut_ptr(), 128, 0);
        if res_tracks != 0 {
            mciSendStringW(close_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);
            return Err(format!(
                "MCI query track count failed with code {res_tracks}"
            ));
        }

        let track_count_str = String::from_utf16_lossy(&buf)
            .trim_matches('\0')
            .trim()
            .to_string();
        let track_count: u8 = track_count_str
            .parse()
            .map_err(|e| format!("Invalid track count from MCI: {track_count_str} ({e})"))?;

        if track_count == 0 || track_count > 99 {
            mciSendStringW(close_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);
            return Err(format!("Invalid track count: {track_count}"));
        }

        let mut offsets = [0u32; 100];
        for t in 1..=track_count {
            let pos_cmd: Vec<u16> = format!("status {alias} position track {t}\0")
                .encode_utf16()
                .collect();
            let mut pos_buf = [0u16; 128];
            let res_pos = mciSendStringW(pos_cmd.as_ptr(), pos_buf.as_mut_ptr(), 128, 0);
            if res_pos == 0 {
                let pos_str = String::from_utf16_lossy(&pos_buf)
                    .trim_matches('\0')
                    .trim()
                    .to_string();
                let parts: Vec<&str> = pos_str.split(':').collect();
                if parts.len() == 3 {
                    let m: u8 = parts[0].parse().unwrap_or(0);
                    let s: u8 = parts[1].parse().unwrap_or(0);
                    let f: u8 = parts[2].parse().unwrap_or(0);
                    offsets[t as usize] = msf_to_sectors(m, s, f);
                }
            }
        }

        let len_cmd: Vec<u16> = format!("status {alias} length\0").encode_utf16().collect();
        let mut len_buf = [0u16; 128];
        let mut leadout = offsets[track_count as usize] + 15000;
        if mciSendStringW(len_cmd.as_ptr(), len_buf.as_mut_ptr(), 128, 0) == 0 {
            let len_str = String::from_utf16_lossy(&len_buf)
                .trim_matches('\0')
                .trim()
                .to_string();
            let parts: Vec<&str> = len_str.split(':').collect();
            if parts.len() == 3 {
                let m: u8 = parts[0].parse().unwrap_or(0);
                let s: u8 = parts[1].parse().unwrap_or(0);
                let f: u8 = parts[2].parse().unwrap_or(0);
                leadout = msf_to_sectors(m, s, f) + offsets[1];
            }
        }

        mciSendStringW(close_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);

        let mut toc = CdToc::new(1, track_count, leadout)?;
        for t in 1..=track_count {
            toc.set_track_offset(t, offsets[t as usize])?;
        }
        Ok(toc)
    }
}

/// Parses the WinCdromToc structure into a platform-independent CdToc.
pub fn parse_win_toc(win_toc: &WinCdromToc) -> Result<CdToc, String> {
    let first_track = win_toc.first_track;
    let last_track = win_toc.last_track;

    if first_track == 0 || last_track == 0 || last_track < first_track || last_track > 99 {
        return Err(format!(
            "Invalid track range reported by disc: {}..={}",
            first_track, last_track
        ));
    }

    let mut audio_tracks = 0u8;
    let mut leadout_offset = 0u32;
    let mut offsets = [0u32; 100];

    let total_entries = (last_track - first_track + 2) as usize; // tracks + leadout

    for i in 0..total_entries {
        if let Some(entry) = win_toc.track_data.get(i) {
            let track_num = entry.track_number;
            let sector = msf_to_sectors(entry.address[1], entry.address[2], entry.address[3]);

            if track_num == 0xAA || i == total_entries - 1 {
                // Lead-out track
                leadout_offset = sector;
            } else if track_num >= first_track && track_num <= last_track {
                // Check if it's an audio track (bit 2 of control byte: 0 = audio, 1 = data)
                let is_data = (entry.control & 0x04) != 0;
                if !is_data {
                    offsets[track_num as usize] = sector;
                    audio_tracks = audio_tracks.max(track_num);
                }
            }
        }
    }

    if audio_tracks < first_track {
        return Err("No audio tracks found in TOC".to_string());
    }

    let mut toc = CdToc::new(first_track, audio_tracks, leadout_offset)?;
    for t in first_track..=audio_tracks {
        toc.set_track_offset(t, offsets[t as usize])?;
    }

    Ok(toc)
}

/// Rips raw CDDA sectors on Windows using IOCTL_CDROM_RAW_READ directly into a writer.
pub fn rip_sectors_windows<W: Write, F: FnMut(f64)>(
    device_path: &str,
    start_sector: u32,
    end_sector: u32,
    cancel_flag: &AtomicBool,
    mut progress_callback: F,
    writer: &mut W,
) -> Result<(), String> {
    if crate::mock_cd::is_mock_device(device_path) {
        return crate::mock_cd::rip_sectors_mock(
            start_sector,
            end_sector,
            cancel_flag,
            progress_callback,
            writer,
        );
    }

    #[cfg(windows)]
    {
        let canonical_path = normalize_device_path(device_path);
        let wide_path: Vec<u16> = canonical_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            CreateFileW(
                wide_path.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                0,
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            return Err(format!(
                "Failed to open CD drive '{canonical_path}' for ripping (Win32 error: {err})"
            ));
        }

        let total_sectors = end_sector.saturating_sub(start_sector);
        if total_sectors == 0 {
            unsafe { CloseHandle(handle) };
            return Err("Track contains 0 sectors".to_string());
        }

        let mut current_sector = start_sector;
        let buffer_size = (MAX_SECTORS_PER_READ * CDDA_BYTES_PER_SECTOR) as usize;
        let mut buffer = vec![0u8; buffer_size];

        while current_sector < end_sector {
            if cancel_flag.load(Ordering::SeqCst) {
                unsafe { CloseHandle(handle) };
                return Err("Ripping cancelled by user".to_string());
            }

            let remaining_sectors = end_sector - current_sector;
            let sectors_to_read = remaining_sectors.min(MAX_SECTORS_PER_READ);

            let raw_read_info = WinRawReadInfo {
                disk_offset: (current_sector as i64) * 2048,
                sector_count: sectors_to_read,
                track_mode: TRACK_MODE_CDDA,
            };

            let bytes_expected = sectors_to_read * CDDA_BYTES_PER_SECTOR;
            let mut bytes_returned: u32 = 0;

            let success = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_CDROM_RAW_READ,
                    &raw_read_info as *const _ as *const _,
                    std::mem::size_of::<WinRawReadInfo>() as u32,
                    buffer.as_mut_ptr() as *mut _,
                    bytes_expected,
                    &mut bytes_returned,
                    std::ptr::null_mut(),
                )
            };

            if success == 0 {
                let err = unsafe { GetLastError() };
                unsafe { CloseHandle(handle) };
                return Err(format!(
                    "Failed reading sector {current_sector} (Win32 error: {err})"
                ));
            }

            let bytes_to_write = bytes_returned as usize;
            if let Err(e) = writer.write_all(&buffer[..bytes_to_write]) {
                unsafe { CloseHandle(handle) };
                return Err(format!("Failed writing audio data to file: {e}"));
            }

            current_sector += sectors_to_read;
            let done_sectors = current_sector.saturating_sub(start_sector);
            let fraction = (done_sectors as f64) / (total_sectors as f64);
            progress_callback(fraction.min(1.0));
        }

        unsafe { CloseHandle(handle) };
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = (
            device_path,
            start_sector,
            end_sector,
            cancel_flag,
            progress_callback,
            writer,
        );
        Err("Win32 CD reader is only available on Windows".to_string())
    }
}

/// Ejects the CD drive tray natively on Windows.
pub fn eject_windows_drive(device_path: &str) -> Result<(), String> {
    if crate::mock_cd::is_mock_device(device_path) {
        return Ok(());
    }

    #[cfg(windows)]
    {
        let drive = extract_drive_root(device_path);

        // 1. Try MCI alias (most compatible and reliable for opening tray door across Windows versions)
        let alias = "oxidisc_eject_tray";
        let open_cmd: Vec<u16> = format!("open {drive} type cdaudio alias {alias}\0")
            .encode_utf16()
            .collect();
        let set_cmd: Vec<u16> = format!("set {alias} door open\0").encode_utf16().collect();
        let close_cmd: Vec<u16> = format!("close {alias}\0").encode_utf16().collect();

        unsafe {
            mciSendStringW(close_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);

            let res_open = mciSendStringW(open_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);
            if res_open == 0 {
                let res_set = mciSendStringW(set_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);
                mciSendStringW(close_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);
                if res_set == 0 {
                    return Ok(());
                }
            }
        }

        // 2. Try generic "set cdaudio door open"
        let cmd: Vec<u16> = "set cdaudio door open\0".encode_utf16().collect();
        let res = unsafe { mciSendStringW(cmd.as_ptr(), std::ptr::null_mut(), 0, 0) };
        if res == 0 {
            return Ok(());
        }

        // 3. Fallback to DeviceIoControl(IOCTL_STORAGE_EJECT_MEDIA) with access 0
        let canonical_path = normalize_device_path(device_path);
        let wide_path: Vec<u16> = canonical_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            CreateFileW(
                wide_path.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                0,
            )
        };

        if handle != INVALID_HANDLE_VALUE {
            let mut bytes_returned: u32 = 0;
            let success = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_STORAGE_EJECT_MEDIA,
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    &mut bytes_returned,
                    std::ptr::null_mut(),
                )
            };
            unsafe { CloseHandle(handle) };

            if success != 0 {
                return Ok(());
            }
        }

        Err(format!("Laufwerk {drive} konnte nicht ausgeworfen werden."))
    }

    #[cfg(not(windows))]
    {
        let _ = device_path;
        Err("Win32 CD eject is only available on Windows".to_string())
    }
}

/// Loads/closes the CD drive tray natively on Windows (pulls the CD in).
pub fn load_windows_drive(device_path: &str) -> Result<(), String> {
    if crate::mock_cd::is_mock_device(device_path) {
        return Ok(());
    }

    #[cfg(windows)]
    {
        let drive = extract_drive_root(device_path);

        // 1. Try MCI alias (most compatible and reliable for closing tray door across Windows versions)
        let alias = "oxidisc_load_tray";
        let open_cmd: Vec<u16> = format!("open {drive} type cdaudio alias {alias}\0")
            .encode_utf16()
            .collect();
        let set_cmd: Vec<u16> = format!("set {alias} door closed\0")
            .encode_utf16()
            .collect();
        let close_cmd: Vec<u16> = format!("close {alias}\0").encode_utf16().collect();

        unsafe {
            mciSendStringW(close_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);

            let res_open = mciSendStringW(open_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);
            if res_open == 0 {
                let res_set = mciSendStringW(set_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);
                mciSendStringW(close_cmd.as_ptr(), std::ptr::null_mut(), 0, 0);
                if res_set == 0 {
                    return Ok(());
                }
            }
        }

        // 2. Try generic "set cdaudio door closed"
        let cmd: Vec<u16> = "set cdaudio door closed\0".encode_utf16().collect();
        let res = unsafe { mciSendStringW(cmd.as_ptr(), std::ptr::null_mut(), 0, 0) };
        if res == 0 {
            return Ok(());
        }

        // 3. Fallback to DeviceIoControl(IOCTL_STORAGE_LOAD_MEDIA) with access 0
        let canonical_path = normalize_device_path(device_path);
        let wide_path: Vec<u16> = canonical_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            CreateFileW(
                wide_path.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                0,
            )
        };

        if handle != INVALID_HANDLE_VALUE {
            let mut bytes_returned: u32 = 0;
            let success = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_STORAGE_LOAD_MEDIA,
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    &mut bytes_returned,
                    std::ptr::null_mut(),
                )
            };
            unsafe { CloseHandle(handle) };

            if success != 0 {
                return Ok(());
            }
        }

        Err(format!("Laufwerk {drive} konnte nicht geschlossen werden."))
    }

    #[cfg(not(windows))]
    {
        let _ = device_path;
        Err("Win32 CD loader is only available on Windows".to_string())
    }
}

// ## Reads up to `duration_secs` of 16-bit stereo PCM audio samples starting from `start_sector` on Windows.
pub fn read_sample_pcm16le_windows(
    device_path: &str,
    start_sector: u32,
    duration_secs: u32,
) -> Result<(String, u32), String> {
    if crate::mock_cd::is_mock_device(device_path) {
        let samples = crate::mock_cd::generate_mock_track_samples(1, duration_secs);
        let fp = crate::acoustid::AcoustidClient::fingerprint_pcm16le(&samples, 44100, 2)?;
        return Ok((fp, duration_secs));
    }

    #[cfg(windows)]
    {
        let canonical_path = normalize_device_path(device_path);
        let wide_path: Vec<u16> = canonical_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            CreateFileW(
                wide_path.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                0,
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            return Err(format!(
                "Failed to open CD drive '{canonical_path}' for acoustic sampling (Win32 error: {err})"
            ));
        }

        let max_sectors_to_sample = (duration_secs.max(1) * 75).min(75 * 120);
        let mut current_sector = start_sector;
        let end_sector = start_sector + max_sectors_to_sample;
        let mut pcm_samples: Vec<i16> = Vec::with_capacity((max_sectors_to_sample as usize) * 588);

        let buffer_size = (MAX_SECTORS_PER_READ * CDDA_BYTES_PER_SECTOR) as usize;
        let mut buffer = vec![0u8; buffer_size];

        while current_sector < end_sector {
            let remaining = end_sector - current_sector;
            let sectors_to_read = remaining.min(MAX_SECTORS_PER_READ);

            let raw_read_info = WinRawReadInfo {
                disk_offset: (current_sector as i64) * 2048,
                sector_count: sectors_to_read,
                track_mode: TRACK_MODE_CDDA,
            };

            let bytes_expected = sectors_to_read * CDDA_BYTES_PER_SECTOR;
            let mut bytes_returned: u32 = 0;

            let success = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_CDROM_RAW_READ,
                    &raw_read_info as *const _ as *const _,
                    std::mem::size_of::<WinRawReadInfo>() as u32,
                    buffer.as_mut_ptr() as *mut _,
                    bytes_expected,
                    &mut bytes_returned,
                    std::ptr::null_mut(),
                )
            };

            if success == 0 {
                break;
            }

            let valid_bytes = (bytes_returned as usize).min(bytes_expected as usize);
            for chunk in buffer[..valid_bytes].chunks_exact(2) {
                pcm_samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
            }

            current_sector += sectors_to_read;
        }

        unsafe { CloseHandle(handle) };

        if pcm_samples.len() < 44100 * 2 {
            return Err("Not enough audio data read for acoustic fingerprinting".to_string());
        }

        let actual_duration_secs = (pcm_samples.len() / (44100 * 2)) as u32;
        let fingerprint =
            crate::acoustid::AcoustidClient::fingerprint_pcm16le(&pcm_samples, 44100, 2)?;

        Ok((fingerprint, actual_duration_secs))
    }

    #[cfg(not(windows))]
    {
        let _ = (device_path, start_sector, duration_secs);
        Err("Win32 CD raw sector sampling is only available on Windows".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msf_to_sectors() {
        assert_eq!(msf_to_sectors(0, 2, 0), 150); // Standard CD 2 second pre-gap = 150 frames
        assert_eq!(msf_to_sectors(1, 0, 0), 4500); // 60 * 75 = 4500
        assert_eq!(msf_to_sectors(3, 24, 63), (3 * 60 + 24) * 75 + 63);
    }

    #[test]
    fn test_normalize_device_path() {
        assert_eq!(normalize_device_path("D:"), r"\\.\D:");
        assert_eq!(normalize_device_path("e:"), r"\\.\E:");
        assert_eq!(normalize_device_path("F:\\"), r"\\.\F:");
        assert_eq!(normalize_device_path(r"\\.\G:"), r"\\.\G:");
        assert_eq!(normalize_device_path(""), r"\\.\D:");
    }

    #[test]
    fn test_parse_win_toc() {
        let mut win_toc: WinCdromToc = unsafe { std::mem::zeroed() };
        win_toc.first_track = 1;
        win_toc.last_track = 2;

        // Track 1: 00:02:00 -> sector 150
        win_toc.track_data[0] = WinTrackData {
            reserved: 0,
            control: 0, // Audio
            track_number: 1,
            reserved1: 0,
            address: [0, 0, 2, 0],
        };

        // Track 2: 03:20:00 -> sector 15000
        win_toc.track_data[1] = WinTrackData {
            reserved: 0,
            control: 0, // Audio
            track_number: 2,
            reserved1: 0,
            address: [0, 3, 20, 0],
        };

        // Lead-out (track 0xAA): 10:00:00 -> sector 45000
        win_toc.track_data[2] = WinTrackData {
            reserved: 0,
            control: 0,
            track_number: 0xAA,
            reserved1: 0,
            address: [0, 10, 0, 0],
        };

        let toc = parse_win_toc(&win_toc).unwrap();
        assert_eq!(toc.first_track, 1);
        assert_eq!(toc.last_track, 2);
        assert_eq!(toc.track_count(), 2);
        assert_eq!(toc.get_track_offset(1), Some(150));
        assert_eq!(toc.get_track_offset(2), Some(15000));
        assert_eq!(toc.leadout_offset, 45000);
    }
}
