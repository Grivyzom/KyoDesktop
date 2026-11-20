use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, WebviewWindow};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct MusicAppInfo {
    pub id: String,
    pub name: String,
    pub process_name: String,
    pub pid: u32,
    pub hwnd: isize,
    pub window_title: String,
    pub is_playing: bool,
    pub track_title: Option<String>,
    pub track_artist: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct MediaTrackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(default)]
    pub genres: Vec<String>,
    pub source_app_id: String,
    pub source_app_name: String,
    pub playback_status: String, // "playing" | "paused" | "stopped" | "idle"
    pub is_playing: bool,
    pub position_secs: Option<u64>,
    pub duration_secs: Option<u64>,
    pub thumbnail_data_uri: Option<String>,
    pub is_shuffle: bool,
    pub repeat_mode: String, // "none" | "track" | "list"
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct QueueTrackItem {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub source_app_name: String,
    pub thumbnail_data_uri: Option<String>,
    pub added_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct MusicQueueStore {
    pub up_next: Vec<QueueTrackItem>,
    pub history: Vec<QueueTrackItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct MusicStateSnapshot {
    pub detected: bool,
    pub is_playing: bool,
    pub current_track: Option<MediaTrackInfo>,
    pub running_apps: Vec<MusicAppInfo>,
    pub up_next: Vec<QueueTrackItem>,
    pub history: Vec<QueueTrackItem>,
    pub sleep_timer_remaining_secs: Option<u64>,
    pub updated_at: u64,
}

static LAST_MUSIC_STATE: OnceLock<Mutex<MusicStateSnapshot>> = OnceLock::new();
static THUMBNAIL_CACHE: OnceLock<Mutex<Option<(String, Option<String>)>>> = OnceLock::new();
static SLEEP_TIMER_DEADLINE: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
static WATCHER_STARTED: AtomicBool = AtomicBool::new(false);

fn state_cache() -> &'static Mutex<MusicStateSnapshot> {
    LAST_MUSIC_STATE.get_or_init(|| Mutex::new(MusicStateSnapshot::default()))
}

fn thumb_cache() -> &'static Mutex<Option<(String, Option<String>)>> {
    THUMBNAIL_CACHE.get_or_init(|| Mutex::new(None))
}

fn sleep_timer_deadline() -> &'static Mutex<Option<u64>> {
    SLEEP_TIMER_DEADLINE.get_or_init(|| Mutex::new(None))
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn load_queue_store() -> MusicQueueStore {
    if let Some(kyo_dir) = crate::get_kyo_data_dir() {
        let path = kyo_dir.join("music_queue.json");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(store) = serde_json::from_str::<MusicQueueStore>(&content) {
                return store;
            }
        }
    }
    MusicQueueStore::default()
}

pub fn save_queue_store(store: &MusicQueueStore) {
    if let Some(kyo_dir) = crate::get_kyo_data_dir() {
        let _ = std::fs::create_dir_all(&kyo_dir);
        let path = kyo_dir.join("music_queue.json");
        if let Ok(json) = serde_json::to_string(store) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &json).is_ok() {
                if std::fs::rename(&tmp, &path).is_err() {
                    let _ = std::fs::write(&path, &json);
                    let _ = std::fs::remove_file(&tmp);
                }
            }
        }
    }
}

/// Registra automáticamente la canción actual en el historial reciente si cambió.
fn record_track_in_history(track: &MediaTrackInfo) -> MusicQueueStore {
    let mut store = load_queue_store();
    if track.title.trim().is_empty() || track.title == "Reproduciendo audio" {
        return store;
    }

    let already_top = store
        .history
        .first()
        .map(|h| h.title.eq_ignore_ascii_case(&track.title) && h.artist.eq_ignore_ascii_case(&track.artist))
        .unwrap_or(false);

    if !already_top {
        // Si la canción que empezó a sonar estaba de primera en `up_next`, consumirla de la cola
        if let Some(pos) = store.up_next.iter().position(|q| {
            q.title.eq_ignore_ascii_case(&track.title)
                || (track.title.to_lowercase().contains(&q.title.to_lowercase()) && !q.title.is_empty())
        }) {
            store.up_next.remove(pos);
        }

        // Eliminar duplicados previos de la misma pista en el historial reciente
        store.history.retain(|h| {
            !(h.title.eq_ignore_ascii_case(&track.title) && h.artist.eq_ignore_ascii_case(&track.artist))
        });

        store.history.insert(
            0,
            QueueTrackItem {
                id: format!("hist-{}-{}", now_unix_secs(), store.history.len()),
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                source_app_name: track.source_app_name.clone(),
                thumbnail_data_uri: track.thumbnail_data_uri.clone(),
                added_at: now_unix_secs(),
            },
        );

        if store.history.len() > 20 {
            store.history.truncate(20);
        }
        save_queue_store(&store);
    } else if let Some(first) = store.history.first_mut() {
        if first.thumbnail_data_uri.is_none() && track.thumbnail_data_uri.is_some() {
            first.thumbnail_data_uri = track.thumbnail_data_uri.clone();
            save_queue_store(&store);
        }
    }

    store
}

/// Identifica una aplicación de música conocida a partir del nombre de proceso o AppUserModelId.
fn classify_music_app(proc_or_aumid: &str, window_title: &str) -> Option<(&'static str, &'static str)> {
    let lower = proc_or_aumid.to_lowercase();
    let title_lower = window_title.to_lowercase();

    if lower.contains("spotify") {
        return Some(("spotify", "Spotify"));
    }
    if lower.contains("applemusic") || lower.contains("itunes") {
        return Some(("apple_music", "Apple Music"));
    }
    if lower.contains("tidal") {
        return Some(("tidal", "TIDAL"));
    }
    if lower.contains("deezer") {
        return Some(("deezer", "Deezer"));
    }
    if lower.contains("aimp") {
        return Some(("aimp", "AIMP"));
    }
    if lower.contains("foobar2000") {
        return Some(("foobar2000", "Foobar2000"));
    }
    if lower.contains("musicbee") {
        return Some(("musicbee", "MusicBee"));
    }
    if lower.contains("winamp") {
        return Some(("winamp", "Winamp"));
    }
    if lower == "vlc.exe" || lower.ends_with("\\vlc.exe") || lower.contains("videolan.vlc") {
        return Some(("vlc", "VLC Media Player"));
    }
    if lower.contains("music.ui") || lower.contains("zunemusic") || lower == "wmplayer.exe" {
        return Some(("windows_media", "Reproductor Multimedia"));
    }

    let is_browser = lower.contains("chrome")
        || lower.contains("msedge")
        || lower.contains("firefox")
        || lower.contains("brave")
        || lower.contains("opera")
        || lower.contains("vivaldi")
        || lower.contains("arc");

    if is_browser {
        if title_lower.contains("youtube music") {
            return Some(("youtube_music", "YouTube Music"));
        }
        if title_lower.contains("spotify") {
            return Some(("spotify_web", "Spotify Web"));
        }
        if title_lower.contains("soundcloud") {
            return Some(("soundcloud", "SoundCloud"));
        }
        if title_lower.contains("apple music") {
            return Some(("apple_music_web", "Apple Music Web"));
        }
        if title_lower.contains("deezer") {
            return Some(("deezer_web", "Deezer Web"));
        }
        if title_lower.contains("tidal") {
            return Some(("tidal_web", "TIDAL Web"));
        }
    }

    None
}

fn parse_window_title_track(app_id: &str, raw_title: &str) -> (Option<String>, Option<String>, bool) {
    let trimmed = raw_title.trim();
    if trimmed.is_empty() {
        return (None, None, false);
    }

    let lower = trimmed.to_lowercase();

    match app_id {
        "spotify" => {
            if lower == "spotify"
                || lower == "spotify free"
                || lower == "spotify premium"
                || lower.starts_with("spotify ")
            {
                return (None, None, false);
            }
            if let Some((artist, song)) = trimmed.split_once(" - ") {
                let a = artist.trim();
                let s = song.trim();
                if !a.is_empty() && !s.is_empty() {
                    return (Some(a.to_string()), Some(s.to_string()), true);
                }
            }
            (None, Some(trimmed.to_string()), true)
        }
        "vlc" => {
            let clean = trimmed
                .strip_suffix(" - VLC media player")
                .or_else(|| trimmed.strip_suffix(" - Reproductor multimedia VLC"))
                .unwrap_or(trimmed)
                .trim();
            if clean.eq_ignore_ascii_case("VLC media player")
                || clean.eq_ignore_ascii_case("Reproductor multimedia VLC")
                || clean.is_empty()
            {
                return (None, None, false);
            }
            if let Some((artist, song)) = clean.split_once(" - ") {
                return (Some(artist.trim().to_string()), Some(song.trim().to_string()), true);
            }
            (None, Some(clean.to_string()), true)
        }
        "foobar2000" => {
            let clean = trimmed
                .split(" [foobar2000")
                .next()
                .unwrap_or(trimmed)
                .trim();
            if clean.eq_ignore_ascii_case("foobar2000") || clean.is_empty() {
                return (None, None, false);
            }
            if let Some((artist, song)) = clean.split_once(" - ") {
                return (Some(artist.trim().to_string()), Some(song.trim().to_string()), true);
            }
            (None, Some(clean.to_string()), true)
        }
        "aimp" | "musicbee" | "winamp" | "tidal" | "deezer" | "apple_music" => {
            if lower == app_id
                || lower == "apple music"
                || lower == "itunes"
                || lower == "aimp"
                || lower == "musicbee"
                || lower == "tidal"
                || lower == "deezer"
            {
                return (None, None, false);
            }
            if let Some((artist, song)) = trimmed.split_once(" - ") {
                return (Some(artist.trim().to_string()), Some(song.trim().to_string()), true);
            }
            (None, None, false)
        }
        "youtube_music" | "spotify_web" | "soundcloud" => {
            let mut clean = trimmed;
            for suffix in [
                " - Google Chrome",
                " - Microsoft​ Edge",
                " - Microsoft Edge",
                " - Mozilla Firefox",
                " - Brave",
                " - Opera",
                " - Vivaldi",
            ] {
                if let Some(stripped) = clean.strip_suffix(suffix) {
                    clean = stripped.trim();
                }
            }
            for platform in [" - YouTube Music", " | YouTube Music", " - SoundCloud", " - Spotify"] {
                if let Some(stripped) = clean.strip_suffix(platform) {
                    clean = stripped.trim();
                }
            }
            if clean.eq_ignore_ascii_case("YouTube Music")
                || clean.eq_ignore_ascii_case("Spotify")
                || clean.eq_ignore_ascii_case("SoundCloud")
                || clean.is_empty()
            {
                return (None, None, false);
            }
            if let Some((song, artist)) = clean.split_once(" • ") {
                return (Some(artist.trim().to_string()), Some(song.trim().to_string()), true);
            }
            if let Some((artist, song)) = clean.split_once(" - ") {
                return (Some(artist.trim().to_string()), Some(song.trim().to_string()), true);
            }
            (None, Some(clean.to_string()), true)
        }
        _ => (None, None, false),
    }
}

#[cfg(windows)]
mod win_impl {
    use super::*;
    use std::collections::HashMap;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionMediaProperties,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    };
    use windows::Media::MediaPlaybackAutoRepeatMode;
    use windows::Storage::Streams::DataReader;

    type HWND = isize;
    type BOOL = i32;

    #[repr(C)]
    struct PROCESSENTRY32W {
        dw_size: u32,
        cnt_usage: u32,
        th32_process_id: u32,
        th32_default_heap_id: usize,
        th32_module_id: u32,
        cnt_threads: u32,
        th32_parent_process_id: u32,
        pc_pri_class_base: i32,
        dw_flags: u32,
        sz_exe_file: [u16; 260],
    }

    const TH32CS_SNAPPROCESS: u32 = 0x00000002;
    const INVALID_HANDLE_VALUE: isize = -1;

    extern "system" {
        fn EnumWindows(lpEnumFunc: unsafe extern "system" fn(HWND, isize) -> BOOL, lParam: isize) -> BOOL;
        fn IsWindowVisible(hWnd: HWND) -> BOOL;
        fn IsIconic(hWnd: HWND) -> BOOL;
        fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
        fn SetForegroundWindow(hWnd: HWND) -> BOOL;
        fn BringWindowToTop(hWnd: HWND) -> BOOL;
        fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
        fn GetClassNameW(hWnd: HWND, lpClassName: *mut u16, nMaxCount: i32) -> i32;
        fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut u32) -> u32;
        fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: BOOL, dwProcessId: u32) -> isize;
        fn CloseHandle(hObject: isize) -> BOOL;
        fn QueryFullProcessImageNameW(hProcess: isize, dwFlags: u32, lpExeName: *mut u16, lpdwSize: *mut u32) -> BOOL;
        fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> isize;
        fn Process32FirstW(hSnapshot: isize, lppe: *mut PROCESSENTRY32W) -> BOOL;
        fn Process32NextW(hSnapshot: isize, lppe: *mut PROCESSENTRY32W) -> BOOL;
        fn keybd_event(bVk: u8, bScan: u8, dwFlags: u32, dwExtraInfo: usize);
        fn GetWindowRect(hWnd: HWND, lpRect: *mut crate::avatar_desktop::win32::RECT) -> BOOL;
    }

    pub const VK_VOLUME_MUTE: u8 = 0xAD;
    pub const VK_VOLUME_DOWN: u8 = 0xAE;
    pub const VK_VOLUME_UP: u8 = 0xAF;
    pub const VK_MEDIA_NEXT_TRACK: u8 = 0xB0;
    pub const VK_MEDIA_PREV_TRACK: u8 = 0xB1;
    pub const VK_MEDIA_STOP: u8 = 0xB2;
    pub const VK_MEDIA_PLAY_PAUSE: u8 = 0xB3;
    const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
    const KEYEVENTF_KEYUP: u32 = 0x0002;

    pub fn send_media_key(vk: u8) {
        unsafe {
            keybd_event(vk, 0, KEYEVENTF_EXTENDEDKEY, 0);
            keybd_event(vk, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
        }
    }

    /// Lee el stream de miniatura (carátula del álbum) desde Windows SMTC y lo codifica como Data URI Base64.
    fn extract_thumbnail_data_uri(
        props: &GlobalSystemMediaTransportControlsSessionMediaProperties,
        cache_key: &str,
    ) -> Option<String> {
        if let Ok(guard) = thumb_cache().lock() {
            if let Some((ref k, ref cached_uri)) = *guard {
                if k == cache_key {
                    return cached_uri.clone();
                }
            }
        }

        let fetch = || -> Option<String> {
            let thumb_ref = props.Thumbnail().ok()?;
            let stream = thumb_ref.OpenReadAsync().ok()?.get().ok()?;
            let size = stream.Size().ok()? as u32;
            if size == 0 || size > 1_500_000 {
                return None;
            }
            let input_stream = stream.GetInputStreamAt(0).ok()?;
            let reader = DataReader::CreateDataReader(&input_stream).ok()?;
            reader.LoadAsync(size).ok()?.get().ok()?;
            let mut bytes = vec![0u8; size as usize];
            reader.ReadBytes(&mut bytes).ok()?;

            let mime = if bytes.starts_with(b"\x89PNG") {
                "image/png"
            } else if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
                "image/webp"
            } else {
                "image/jpeg"
            };

            let b64 = crate::token_crypto::base64_encode(&bytes);
            Some(format!("data:{};base64,{}", mime, b64))
        };

        let result = fetch();
        if let Ok(mut guard) = thumb_cache().lock() {
            *guard = Some((cache_key.to_string(), result.clone()));
        }
        result
    }

    pub fn scan_music_windows_and_processes() -> Vec<MusicAppInfo> {
        struct EnumCtx {
            found: HashMap<String, MusicAppInfo>,
        }

        unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: isize) -> BOOL {
            let ctx = &mut *(lparam as *mut EnumCtx);
            if IsWindowVisible(hwnd) == 0 {
                return 1;
            }

            let mut class_buf = [0u16; 256];
            let class_len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), 256);
            if class_len > 0 {
                let cname = String::from_utf16_lossy(&class_buf[..class_len as usize]).to_lowercase();
                if cname.contains("tooltip") || cname.contains("ime") || cname == "progman" {
                    return 1;
                }
            }

            let mut title_buf = [0u16; 512];
            let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 512);
            if title_len <= 0 {
                return 1;
            }
            let window_title = String::from_utf16_lossy(&title_buf[..title_len as usize]).trim().to_string();
            if window_title.is_empty()
                || window_title == "Default IME"
                || window_title == "MSCTFIME UI"
                || window_title == "GDI+ Window"
            {
                return 1;
            }

            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 {
                return 1;
            }

            let mut proc_name = String::new();
            let hproc = OpenProcess(0x1000, 0, pid);
            if hproc != 0 {
                let mut path_buf = [0u16; 1024];
                let mut path_len = 1024u32;
                if QueryFullProcessImageNameW(hproc, 0, path_buf.as_mut_ptr(), &mut path_len) != 0 {
                    let full_path = String::from_utf16_lossy(&path_buf[..path_len as usize]);
                    if let Some(fname) = std::path::Path::new(&full_path).file_name() {
                        proc_name = fname.to_string_lossy().to_string();
                    }
                }
                CloseHandle(hproc);
            }

            if proc_name.is_empty() {
                return 1;
            }

            if let Some((app_id, app_name)) = classify_music_app(&proc_name, &window_title) {
                let (artist, title, is_playing) = parse_window_title_track(app_id, &window_title);

                let entry = MusicAppInfo {
                    id: app_id.to_string(),
                    name: app_name.to_string(),
                    process_name: proc_name,
                    pid,
                    hwnd,
                    window_title,
                    is_playing,
                    track_title: title,
                    track_artist: artist,
                };

                if let Some(existing) = ctx.found.get(app_id) {
                    if !existing.is_playing && entry.is_playing {
                        ctx.found.insert(app_id.to_string(), entry);
                    }
                } else {
                    ctx.found.insert(app_id.to_string(), entry);
                }
            }

            1
        }

        let mut ctx = EnumCtx {
            found: HashMap::new(),
        };

        unsafe {
            EnumWindows(enum_cb, &mut ctx as *mut EnumCtx as isize);

            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap != INVALID_HANDLE_VALUE && snap != 0 {
                let mut pe: PROCESSENTRY32W = std::mem::zeroed();
                pe.dw_size = std::mem::size_of::<PROCESSENTRY32W>() as u32;
                if Process32FirstW(snap, &mut pe) != 0 {
                    loop {
                        let len = pe.sz_exe_file.iter().position(|&c| c == 0).unwrap_or(pe.sz_exe_file.len());
                        let exe_name = String::from_utf16_lossy(&pe.sz_exe_file[..len]);
                        let lower = exe_name.to_lowercase();
                        let is_dedicated_music = lower == "spotify.exe"
                            || lower == "applemusic.exe"
                            || lower == "itunes.exe"
                            || lower == "tidal.exe"
                            || lower == "deezer.exe"
                            || lower == "aimp.exe"
                            || lower == "foobar2000.exe"
                            || lower == "musicbee.exe"
                            || lower == "winamp.exe"
                            || lower == "vlc.exe";

                        if is_dedicated_music {
                            if let Some((app_id, app_name)) = classify_music_app(&exe_name, "") {
                                ctx.found.entry(app_id.to_string()).or_insert_with(|| MusicAppInfo {
                                    id: app_id.to_string(),
                                    name: app_name.to_string(),
                                    process_name: exe_name,
                                    pid: pe.th32_process_id,
                                    hwnd: 0,
                                    window_title: app_name.to_string(),
                                    is_playing: false,
                                    track_title: None,
                                    track_artist: None,
                                });
                            }
                        }

                        if Process32NextW(snap, &mut pe) == 0 {
                            break;
                        }
                    }
                }
                CloseHandle(snap);
            }
        }

        let mut list: Vec<MusicAppInfo> = ctx.found.into_values().collect();
        list.sort_by(|a, b| b.is_playing.cmp(&a.is_playing).then_with(|| a.name.cmp(&b.name)));
        list
    }

    pub fn query_smtc_session() -> Option<MediaTrackInfo> {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync().ok()?.get().ok()?;

        let mut candidate_session = None;

        if let Ok(sessions) = manager.GetSessions() {
            let count = sessions.Size().unwrap_or(0);
            for i in 0..count {
                if let Ok(session) = sessions.GetAt(i) {
                    if let Ok(info) = session.GetPlaybackInfo() {
                        if let Ok(status) = info.PlaybackStatus() {
                            if status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing {
                                candidate_session = Some(session);
                                break;
                            }
                        }
                    }
                }
            }
        }

        if candidate_session.is_none() {
            candidate_session = manager.GetCurrentSession().ok();
        }

        let session = candidate_session?;
        let aumid = session
            .SourceAppUserModelId()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();

        let playback_info = session.GetPlaybackInfo().ok()?;
        let raw_status = playback_info.PlaybackStatus().ok()?;

        let (status_str, is_playing) = match raw_status {
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => ("playing", true),
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => ("paused", false),
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped => ("stopped", false),
            _ => ("idle", false),
        };

        let is_shuffle = playback_info
            .IsShuffleActive()
            .ok()
            .and_then(|r| r.Value().ok())
            .unwrap_or(false);

        let repeat_mode = playback_info
            .AutoRepeatMode()
            .ok()
            .and_then(|r| r.Value().ok())
            .map(|m| match m {
                MediaPlaybackAutoRepeatMode::Track => "track",
                MediaPlaybackAutoRepeatMode::List => "list",
                _ => "none",
            })
            .unwrap_or("none")
            .to_string();

        let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
        let title = props.Title().map(|s| s.to_string_lossy()).unwrap_or_default().trim().to_string();
        let artist = props.Artist().map(|s| s.to_string_lossy()).unwrap_or_default().trim().to_string();
        let album = props.AlbumTitle().map(|s| s.to_string_lossy()).unwrap_or_default().trim().to_string();
        let mut genres = Vec::new();
        if let Ok(g_list) = props.Genres() {
            let g_count = g_list.Size().unwrap_or(0);
            for idx in 0..g_count {
                if let Ok(g) = g_list.GetAt(idx) {
                    let s = g.to_string_lossy().trim().to_string();
                    if !s.is_empty() {
                        genres.push(s);
                    }
                }
            }
        }

        if title.is_empty() && artist.is_empty() && !is_playing {
            return None;
        }

        let (position_secs, duration_secs) = if let Ok(timeline) = session.GetTimelineProperties() {
            let pos = timeline.Position().ok().map(|d| (d.Duration.max(0) as u64) / 10_000_000);
            let end = timeline.EndTime().ok().map(|d| (d.Duration.max(0) as u64) / 10_000_000);
            (pos, end.filter(|&e| e > 0))
        } else {
            (None, None)
        };

        let (app_id, app_name) = classify_music_app(&aumid, &title).unwrap_or_else(|| {
            let l = aumid.to_lowercase();
            if l.contains("chrome") || l.contains("msedge") || l.contains("firefox") || l.contains("brave") {
                ("browser_media", "Navegador Web")
            } else {
                ("system_media", "Multimedia del Sistema")
            }
        });

        let cache_key = format!("{}::{}::{}", app_id, artist, title);
        let thumbnail_data_uri = extract_thumbnail_data_uri(&props, &cache_key);

        Some(MediaTrackInfo {
            title: if title.is_empty() { "Reproduciendo audio".to_string() } else { title },
            artist,
            album,
            genres,
            source_app_id: app_id.to_string(),
            source_app_name: app_name.to_string(),
            playback_status: status_str.to_string(),
            is_playing,
            position_secs,
            duration_secs,
            thumbnail_data_uri,
            is_shuffle,
            repeat_mode,
        })
    }

    pub fn execute_smtc_or_key_action(action: &str) -> Result<String, String> {
        if let Some(secs_str) = action.strip_prefix("seek:") {
            if let Ok(target_secs) = secs_str.trim().parse::<u64>() {
                let seek_ticks = (target_secs as i64) * 10_000_000;
                if let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
                    if let Ok(manager) = op.get() {
                        if let Ok(session) = manager.GetCurrentSession() {
                            if let Ok(seek_op) = session.TryChangePlaybackPositionAsync(seek_ticks) {
                                let _ = seek_op.get();
                            }
                        }
                    }
                }
                return Ok("seek".to_string());
            }
        }

        if action == "toggle_shuffle" {
            if let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
                if let Ok(manager) = op.get() {
                    if let Ok(session) = manager.GetCurrentSession() {
                        let cur = session
                            .GetPlaybackInfo()
                            .ok()
                            .and_then(|i| i.IsShuffleActive().ok())
                            .and_then(|r| r.Value().ok())
                            .unwrap_or(false);
                        if let Ok(shuf_op) = session.TryChangeShuffleActiveAsync(!cur) {
                            let _ = shuf_op.get();
                        }
                    }
                }
            }
            return Ok("toggle_shuffle".to_string());
        }

        if action == "cycle_repeat" {
            if let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
                if let Ok(manager) = op.get() {
                    if let Ok(session) = manager.GetCurrentSession() {
                        let cur = session
                            .GetPlaybackInfo()
                            .ok()
                            .and_then(|i| i.AutoRepeatMode().ok())
                            .and_then(|r| r.Value().ok())
                            .unwrap_or(MediaPlaybackAutoRepeatMode::None);
                        let next_mode = match cur {
                            MediaPlaybackAutoRepeatMode::None => MediaPlaybackAutoRepeatMode::List,
                            MediaPlaybackAutoRepeatMode::List => MediaPlaybackAutoRepeatMode::Track,
                            _ => MediaPlaybackAutoRepeatMode::None,
                        };
                        if let Ok(rep_op) = session.TryChangeAutoRepeatModeAsync(next_mode) {
                            let _ = rep_op.get();
                        }
                    }
                }
            }
            return Ok("cycle_repeat".to_string());
        }

        let try_smtc = || -> Option<bool> {
            let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync().ok()?.get().ok()?;
            let session = manager.GetCurrentSession().ok()?;
            let ok = match action {
                "play_pause" => session.TryTogglePlayPauseAsync().ok()?.get().ok()?,
                "play" => session.TryPlayAsync().ok()?.get().ok()?,
                "pause" => session.TryPauseAsync().ok()?.get().ok()?,
                "next" => session.TrySkipNextAsync().ok()?.get().ok()?,
                "previous" => session.TrySkipPreviousAsync().ok()?.get().ok()?,
                "stop" => session.TryStopAsync().ok()?.get().ok()?,
                _ => return None,
            };
            Some(ok)
        };

        match action {
            "play_pause" => {
                if try_smtc() != Some(true) {
                    send_media_key(VK_MEDIA_PLAY_PAUSE);
                }
                Ok("play_pause".to_string())
            }
            "play" => {
                if try_smtc() != Some(true) {
                    send_media_key(VK_MEDIA_PLAY_PAUSE);
                }
                Ok("play".to_string())
            }
            "pause" => {
                if try_smtc() != Some(true) {
                    send_media_key(VK_MEDIA_PLAY_PAUSE);
                }
                Ok("pause".to_string())
            }
            "next" => {
                if try_smtc() != Some(true) {
                    send_media_key(VK_MEDIA_NEXT_TRACK);
                }
                Ok("next".to_string())
            }
            "previous" => {
                if try_smtc() != Some(true) {
                    send_media_key(VK_MEDIA_PREV_TRACK);
                }
                Ok("previous".to_string())
            }
            "stop" => {
                if try_smtc() != Some(true) {
                    send_media_key(VK_MEDIA_STOP);
                }
                Ok("stop".to_string())
            }
            "volume_up" => {
                send_media_key(VK_VOLUME_UP);
                Ok("volume_up".to_string())
            }
            "volume_down" => {
                send_media_key(VK_VOLUME_DOWN);
                Ok("volume_down".to_string())
            }
            "volume_mute" => {
                send_media_key(VK_VOLUME_MUTE);
                Ok("volume_mute".to_string())
            }
            _ => Err(format!("Acción multimedia desconocida: {}", action)),
        }
    }

    pub fn focus_music_window(hwnd: isize) -> bool {
        if hwnd == 0 {
            return false;
        }
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, 9);
            } else {
                ShowWindow(hwnd, 5);
            }
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd) != 0
        }
    }

    pub fn get_window_rect(hwnd: isize) -> Option<crate::avatar_desktop::win32::RECT> {
        if hwnd == 0 {
            return None;
        }
        unsafe {
            let mut rc = crate::avatar_desktop::win32::RECT::default();
            if GetWindowRect(hwnd, &mut rc) != 0 && rc.right > rc.left && rc.bottom > rc.top {
                Some(rc)
            } else {
                None
            }
        }
    }
}

pub fn launch_spotify_search_or_uri(query: &str) {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return;
    }
    let encoded: String = trimmed
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c.to_string(),
            ' ' => "%20".to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect();
    let uri = if trimmed.starts_with("spotify:") {
        trimmed.to_string()
    } else {
        format!("spotify:search:{encoded}")
    };
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &uri])
            .spawn();
    }
}

/// Construye el snapshot combinado (SMTC + Procesos y Ventanas + Cola + Historial + Temporizador).
pub fn collect_music_snapshot() -> MusicStateSnapshot {
    #[cfg(windows)]
    {
        // Verificar temporizador de apagado musical (Sleep Timer)
        let now_ts = now_unix_secs();
        let mut sleep_remaining = None;
        let mut timer_expired = false;
        if let Ok(mut guard) = sleep_timer_deadline().lock() {
            if let Some(deadline) = *guard {
                if now_ts >= deadline {
                    *guard = None;
                    timer_expired = true;
                } else {
                    sleep_remaining = Some(deadline - now_ts);
                }
            }
        }
        if timer_expired {
            let _ = win_impl::execute_smtc_or_key_action("pause");
        }

        let mut apps = win_impl::scan_music_windows_and_processes();
        let mut track = win_impl::query_smtc_session();

        if let Some(ref smtc_track) = track {
            let mut matched = false;
            for app in apps.iter_mut() {
                if app.id == smtc_track.source_app_id
                    || (app.id == "spotify" && smtc_track.source_app_id == "spotify")
                {
                    app.is_playing = smtc_track.is_playing;
                    if !smtc_track.title.is_empty() {
                        app.track_title = Some(smtc_track.title.clone());
                    }
                    if !smtc_track.artist.is_empty() {
                        app.track_artist = Some(smtc_track.artist.clone());
                    }
                    matched = true;
                }
            }
            if !matched && (smtc_track.is_playing || !smtc_track.title.is_empty()) {
                apps.push(MusicAppInfo {
                    id: smtc_track.source_app_id.clone(),
                    name: smtc_track.source_app_name.clone(),
                    process_name: smtc_track.source_app_name.clone(),
                    pid: 0,
                    hwnd: 0,
                    window_title: if smtc_track.artist.is_empty() {
                        smtc_track.title.clone()
                    } else {
                        format!("{} - {}", smtc_track.artist, smtc_track.title)
                    },
                    is_playing: smtc_track.is_playing,
                    track_title: Some(smtc_track.title.clone()),
                    track_artist: if smtc_track.artist.is_empty() {
                        None
                    } else {
                        Some(smtc_track.artist.clone())
                    },
                });
            }
        }

        let smtc_is_playing = track.as_ref().map(|t| t.is_playing).unwrap_or(false);
        if !smtc_is_playing {
            if let Some(playing_app) = apps.iter().find(|a| a.is_playing && a.track_title.is_some()) {
                track = Some(MediaTrackInfo {
                    title: playing_app.track_title.clone().unwrap_or_default(),
                    artist: playing_app.track_artist.clone().unwrap_or_default(),
                    album: String::new(),
                    genres: Vec::new(),
                    source_app_id: playing_app.id.clone(),
                    source_app_name: playing_app.name.clone(),
                    playback_status: "playing".to_string(),
                    is_playing: true,
                    position_secs: None,
                    duration_secs: None,
                    thumbnail_data_uri: None,
                    is_shuffle: false,
                    repeat_mode: "none".to_string(),
                });
            }
        }

        apps.sort_by(|a, b| b.is_playing.cmp(&a.is_playing).then_with(|| a.name.cmp(&b.name)));

        let store = if let Some(ref tr) = track {
            record_track_in_history(tr)
        } else {
            load_queue_store()
        };

        let is_playing = track.as_ref().map(|t| t.is_playing).unwrap_or(false)
            || apps.iter().any(|a| a.is_playing);
        let detected = !apps.is_empty() || track.is_some();

        MusicStateSnapshot {
            detected,
            is_playing,
            current_track: track,
            running_apps: apps,
            up_next: store.up_next,
            history: store.history,
            sleep_timer_remaining_secs: sleep_remaining,
            updated_at: now_ts,
        }
    }

    #[cfg(not(windows))]
    {
        MusicStateSnapshot {
            detected: false,
            is_playing: false,
            current_track: None,
            running_apps: Vec::new(),
            up_next: Vec::new(),
            history: Vec::new(),
            sleep_timer_remaining_secs: None,
            updated_at: now_unix_secs(),
        }
    }
}

fn persist_music_snapshot(snapshot: &MusicStateSnapshot) {
    if let Some(kyo_dir) = crate::get_kyo_data_dir() {
        let _ = std::fs::create_dir_all(&kyo_dir);
        let path = kyo_dir.join("music_state.json");
        if let Ok(json) = serde_json::to_string(snapshot) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &json).is_ok() {
                if std::fs::rename(&tmp, &path).is_err() {
                    let _ = std::fs::write(&path, &json);
                    let _ = std::fs::remove_file(&tmp);
                }
            }
        }
    }
}

fn snapshots_meaningfully_different(a: &MusicStateSnapshot, b: &MusicStateSnapshot) -> bool {
    if a.detected != b.detected
        || a.is_playing != b.is_playing
        || a.up_next.len() != b.up_next.len()
        || a.history.len() != b.history.len()
        || a.sleep_timer_remaining_secs.is_some() != b.sleep_timer_remaining_secs.is_some()
    {
        return true;
    }
    if a.running_apps.len() != b.running_apps.len() {
        return true;
    }
    match (&a.current_track, &b.current_track) {
        (Some(ta), Some(tb)) => {
            ta.title != tb.title
                || ta.artist != tb.artist
                || ta.album != tb.album
                || ta.source_app_id != tb.source_app_id
                || ta.playback_status != tb.playback_status
                || ta.is_playing != tb.is_playing
                || ta.is_shuffle != tb.is_shuffle
                || ta.repeat_mode != tb.repeat_mode
                || ta.position_secs != tb.position_secs
        }
        (None, None) => false,
        _ => true,
    }
}

pub fn start_music_watcher(app: &AppHandle) {
    if WATCHER_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let app_handle = app.clone();
    std::thread::spawn(move || {
        let initial = collect_music_snapshot();
        persist_music_snapshot(&initial);
        if let Ok(mut guard) = state_cache().lock() {
            *guard = initial.clone();
        }
        let _ = app_handle.emit("music://state-changed", &initial);

        let mut tick_count: u64 = 0;

        loop {
            std::thread::sleep(Duration::from_millis(1200));
            tick_count = tick_count.wrapping_add(1);

            let fresh = collect_music_snapshot();
            let mut changed = false;

            if let Ok(mut guard) = state_cache().lock() {
                if snapshots_meaningfully_different(&guard, &fresh) {
                    changed = true;
                }
                *guard = fresh.clone();
            }

            if changed {
                persist_music_snapshot(&fresh);
                let _ = app_handle.emit("music://state-changed", &fresh);
            } else if tick_count % 5 == 0 {
                persist_music_snapshot(&fresh);
            }
        }
    });
}

#[tauri::command]
pub fn get_music_state(refresh: Option<bool>) -> MusicStateSnapshot {
    if refresh.unwrap_or(false) {
        let fresh = collect_music_snapshot();
        persist_music_snapshot(&fresh);
        if let Ok(mut guard) = state_cache().lock() {
            *guard = fresh.clone();
        }
        return fresh;
    }
    if let Ok(guard) = state_cache().lock() {
        if guard.updated_at > 0 {
            return guard.clone();
        }
    }
    let fresh = collect_music_snapshot();
    persist_music_snapshot(&fresh);
    if let Ok(mut guard) = state_cache().lock() {
        *guard = fresh.clone();
    }
    fresh
}

#[tauri::command]
pub async fn control_music(
    app: AppHandle,
    window: WebviewWindow,
    action: String,
    target_app_id: Option<String>,
) -> Result<MusicStateSnapshot, String> {
    #[cfg(windows)]
    {
        if let Some(query) = action.strip_prefix("queue_add:") {
            let parts: Vec<&str> = query.splitn(2, " - ").collect();
            let (artist, title) = if parts.len() == 2 {
                (parts[0].trim().to_string(), parts[1].trim().to_string())
            } else {
                ("Spotify".to_string(), query.trim().to_string())
            };
            if !title.is_empty() {
                let mut store = load_queue_store();
                store.up_next.push(QueueTrackItem {
                    id: format!("q-{}-{}", now_unix_secs(), store.up_next.len()),
                    title,
                    artist,
                    album: String::new(),
                    source_app_name: "Spotify".to_string(),
                    thumbnail_data_uri: None,
                    added_at: now_unix_secs(),
                });
                save_queue_store(&store);
            }
        } else if let Some(id_to_remove) = action.strip_prefix("queue_remove:") {
            let mut store = load_queue_store();
            store.up_next.retain(|item| item.id != id_to_remove.trim());
            save_queue_store(&store);
        } else if action == "queue_clear" {
            let mut store = load_queue_store();
            store.up_next.clear();
            save_queue_store(&store);
        } else if let Some(query) = action.strip_prefix("play_query:") {
            launch_spotify_search_or_uri(query);
        } else if let Some(mins_str) = action.strip_prefix("sleep_timer:") {
            let mins = mins_str.trim().parse::<u64>().unwrap_or(0);
            if let Ok(mut guard) = sleep_timer_deadline().lock() {
                if mins == 0 {
                    *guard = None;
                } else {
                    *guard = Some(now_unix_secs() + mins * 60);
                }
            }
        } else {
            match action.as_str() {
                "focus_app" => {
                    let snap = collect_music_snapshot();
                    let target = if let Some(ref tid) = target_app_id {
                        snap.running_apps.iter().find(|a| &a.id == tid && a.hwnd != 0)
                    } else {
                        snap.running_apps
                            .iter()
                            .find(|a| a.is_playing && a.hwnd != 0)
                            .or_else(|| snap.running_apps.iter().find(|a| a.hwnd != 0))
                    };

                    if let Some(app_info) = target {
                        win_impl::focus_music_window(app_info.hwnd);
                    } else if snap.running_apps.iter().any(|a| a.id.contains("spotify")) {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", "spotify:"])
                            .spawn();
                    }
                }
                "move_near_app" => {
                    let snap = collect_music_snapshot();
                    let target = if let Some(ref tid) = target_app_id {
                        snap.running_apps.iter().find(|a| &a.id == tid && a.hwnd != 0)
                    } else {
                        snap.running_apps
                            .iter()
                            .find(|a| a.is_playing && a.hwnd != 0)
                            .or_else(|| snap.running_apps.iter().find(|a| a.hwnd != 0))
                    };

                    if let Some(app_info) = target {
                        if let Some(rc) = win_impl::get_window_rect(app_info.hwnd) {
                            if let Ok(bounds) = crate::avatar_desktop::get_desktop_bounds(window.clone()) {
                                let win_w = bounds.window_width;
                                let win_h = bounds.window_height;
                                let margin = (16.0 * bounds.scale_factor) as i32;

                                let raw_x = (rc.right - win_w - margin).clamp(
                                    bounds.work_area.left + margin,
                                    (bounds.work_area.right - win_w - margin).max(bounds.work_area.left),
                                );
                                let raw_y = (rc.bottom - win_h - margin).clamp(
                                    bounds.work_area.top + margin,
                                    (bounds.work_area.bottom - win_h - margin).max(bounds.work_area.top),
                                );

                                let _ = crate::avatar_desktop::animate_avatar_to(
                                    window.clone(),
                                    raw_x,
                                    raw_y,
                                    750,
                                    Some("smooth".to_string()),
                                );
                            }
                        }
                    }
                }
                _ => {
                    win_impl::execute_smtc_or_key_action(&action)?;
                }
            }
        }

        std::thread::sleep(Duration::from_millis(160));
        let updated = collect_music_snapshot();
        persist_music_snapshot(&updated);
        if let Ok(mut guard) = state_cache().lock() {
            *guard = updated.clone();
        }
        let _ = app.emit("music://state-changed", &updated);
        return Ok(updated);
    }

    #[cfg(not(windows))]
    {
        let _ = (app, window, action, target_app_id);
        Ok(collect_music_snapshot())
    }
}
