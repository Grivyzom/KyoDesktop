//! Reproductor Musical Inteligente con Temática Spotify, Fila de Reproducción (Cola + Historial),
//! Carátula en Vivo (SMTC Thumbnail), Barra de Progreso Interactiva, Temporizador y DJ Kyo IA.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub source_app_id: String,
    pub source_app_name: String,
    pub playback_status: String,
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

static THUMBNAIL_CACHE: OnceLock<Mutex<Option<(String, Option<String>)>>> = OnceLock::new();
static SLEEP_TIMER_DEADLINE: OnceLock<Mutex<Option<u64>>> = OnceLock::new();

fn thumb_cache() -> &'static Mutex<Option<(String, Option<String>)>> {
    THUMBNAIL_CACHE.get_or_init(|| Mutex::new(None))
}

fn sleep_timer_deadline() -> &'static Mutex<Option<u64>> {
    SLEEP_TIMER_DEADLINE.get_or_init(|| Mutex::new(None))
}

impl MusicStateSnapshot {
    pub fn primary_app_name(&self) -> String {
        if let Some(ref tr) = self.current_track {
            if !tr.source_app_name.is_empty() {
                return tr.source_app_name.clone();
            }
        }
        if let Some(first) = self.running_apps.first() {
            return first.name.clone();
        }
        "Spotify".to_string()
    }

    pub fn primary_app_id(&self) -> String {
        if let Some(ref tr) = self.current_track {
            if !tr.source_app_id.is_empty() {
                return tr.source_app_id.clone();
            }
        }
        if let Some(first) = self.running_apps.first() {
            return first.id.clone();
        }
        "spotify".to_string()
    }

    /// Genera una descripción completa de contexto musical (pista + fila + historial) para el agente de IA.
    pub fn to_agent_context_line(&self) -> Option<String> {
        if !self.detected {
            return None;
        }
        let mut parts = Vec::new();
        if let Some(ref tr) = self.current_track {
            let status_es = if tr.is_playing { "Reproduciendo ahora" } else { "En pausa" };
            let artist_part = if tr.artist.is_empty() {
                String::new()
            } else {
                format!(" de \"{}\"", tr.artist)
            };
            let album_part = if tr.album.is_empty() {
                String::new()
            } else {
                format!(" (Álbum: \"{}\")", tr.album)
            };
            parts.push(format!(
                "App={} | Estado={} | Canción=\"{}\"{}{}",
                tr.source_app_name, status_es, tr.title, artist_part, album_part
            ));
        }
        if !self.up_next.is_empty() {
            let next_str: Vec<String> = self
                .up_next
                .iter()
                .take(4)
                .map(|q| format!("\"{}\" ({})", q.title, q.artist))
                .collect();
            parts.push(format!("Fila a continuación=[{}]", next_str.join(", ")));
        }
        if !self.history.is_empty() {
            let hist_str: Vec<String> = self
                .history
                .iter()
                .take(4)
                .map(|h| format!("\"{}\" ({})", h.title, h.artist))
                .collect();
            parts.push(format!("Escuchado recientemente=[{}]", hist_str.join(", ")));
        }
        if parts.is_empty() && !self.running_apps.is_empty() {
            let names: Vec<String> = self.running_apps.iter().map(|a| a.name.clone()).collect();
            parts.push(format!("Apps abiertas={}", names.join(", ")));
        }
        if parts.is_empty() {
            None
        } else {
            Some(format!("[Contexto Musical del PC: {}]", parts.join(" | ")))
        }
    }
}

/// Estado reactivo compartido de música en toda la aplicación Dioxus.
#[derive(Clone, Copy, PartialEq)]
pub struct MusicContext {
    pub state: Signal<MusicStateSnapshot>,
    pub pending_chat_prompt: Signal<Option<String>>,
}

impl MusicContext {
    pub fn new() -> Self {
        Self {
            state: Signal::new(MusicStateSnapshot::default()),
            pending_chat_prompt: Signal::new(None),
        }
    }
}

pub fn use_music_context() -> MusicContext {
    match try_use_context::<MusicContext>() {
        Some(ctx) => ctx,
        None => use_context_provider(MusicContext::new),
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn get_kyo_data_dir() -> Option<std::path::PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return Some(std::path::PathBuf::from(appdata).join("kyo"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(std::path::PathBuf::from(home).join(".config").join("kyo"));
    }
    None
}

pub fn load_queue_store() -> MusicQueueStore {
    if let Some(dir) = get_kyo_data_dir() {
        let path = dir.join("music_queue.json");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(store) = serde_json::from_str::<MusicQueueStore>(&content) {
                return store;
            }
        }
    }
    MusicQueueStore::default()
}

pub fn save_queue_store(store: &MusicQueueStore) {
    if let Some(dir) = get_kyo_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("music_queue.json");
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
        if let Some(pos) = store.up_next.iter().position(|q| {
            q.title.eq_ignore_ascii_case(&track.title)
                || (track.title.to_lowercase().contains(&q.title.to_lowercase()) && !q.title.is_empty())
        }) {
            store.up_next.remove(pos);
        }

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
        _ => {
            if let Some((artist, song)) = trimmed.split_once(" - ") {
                return (Some(artist.trim().to_string()), Some(song.trim().to_string()), true);
            }
            (None, None, false)
        }
    }
}

#[cfg(windows)]
mod win_impl {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
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

            let b64 = B64_STD.encode(&bytes);
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
            if window_title.is_empty() {
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
                        let is_dedicated = lower == "spotify.exe"
                            || lower == "applemusic.exe"
                            || lower == "itunes.exe"
                            || lower == "tidal.exe"
                            || lower == "deezer.exe"
                            || lower == "aimp.exe"
                            || lower == "foobar2000.exe"
                            || lower == "musicbee.exe"
                            || lower == "winamp.exe"
                            || lower == "vlc.exe";
                        if is_dedicated {
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

    pub fn execute_media_action(action: &str) {
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
                return;
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
            return;
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
            return;
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
            "play_pause" | "play" | "pause" => {
                if try_smtc() != Some(true) {
                    send_media_key(VK_MEDIA_PLAY_PAUSE);
                }
            }
            "next" => {
                if try_smtc() != Some(true) {
                    send_media_key(VK_MEDIA_NEXT_TRACK);
                }
            }
            "previous" => {
                if try_smtc() != Some(true) {
                    send_media_key(VK_MEDIA_PREV_TRACK);
                }
            }
            "stop" => {
                if try_smtc() != Some(true) {
                    send_media_key(VK_MEDIA_STOP);
                }
            }
            "volume_up" => send_media_key(VK_VOLUME_UP),
            "volume_down" => send_media_key(VK_VOLUME_DOWN),
            "volume_mute" => send_media_key(VK_VOLUME_MUTE),
            _ => {}
        }
    }

    pub fn focus_music_window(hwnd: isize) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, 9);
            } else {
                ShowWindow(hwnd, 5);
            }
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);
        }
    }
}

pub fn collect_music_snapshot_sync() -> MusicStateSnapshot {
    #[cfg(windows)]
    {
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
            win_impl::execute_media_action("pause");
        }

        let mut apps = win_impl::scan_music_windows_and_processes();
        let mut track = win_impl::query_smtc_session();

        if let Some(ref smtc_track) = track {
            let mut matched = false;
            for app in apps.iter_mut() {
                if app.id == smtc_track.source_app_id {
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
                    window_title: smtc_track.title.clone(),
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
        MusicStateSnapshot::default()
    }
}

pub fn trigger_music_action(action: impl Into<String>, mut state_sig: Signal<MusicStateSnapshot>) {
    let action_str = action.into();
    spawn(async move {
        let updated = tokio::task::spawn_blocking(move || {
            #[cfg(windows)]
            {
                if let Some(query) = action_str.strip_prefix("queue_add:") {
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
                } else if let Some(id_to_remove) = action_str.strip_prefix("queue_remove:") {
                    let mut store = load_queue_store();
                    store.up_next.retain(|item| item.id != id_to_remove.trim());
                    save_queue_store(&store);
                } else if action_str == "queue_clear" {
                    let mut store = load_queue_store();
                    store.up_next.clear();
                    save_queue_store(&store);
                } else if let Some(query) = action_str.strip_prefix("play_query:") {
                    launch_spotify_search_or_uri(query);
                } else if let Some(mins_str) = action_str.strip_prefix("sleep_timer:") {
                    let mins = mins_str.trim().parse::<u64>().unwrap_or(0);
                    if let Ok(mut guard) = sleep_timer_deadline().lock() {
                        if mins == 0 {
                            *guard = None;
                        } else {
                            *guard = Some(now_unix_secs() + mins * 60);
                        }
                    }
                } else if action_str == "focus_app" {
                    let cur = collect_music_snapshot_sync();
                    if let Some(app) = cur
                        .running_apps
                        .iter()
                        .find(|a| a.is_playing && a.hwnd != 0)
                        .or_else(|| cur.running_apps.iter().find(|a| a.hwnd != 0))
                    {
                        win_impl::focus_music_window(app.hwnd);
                    } else if cur.running_apps.iter().any(|a| a.id.contains("spotify")) {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", "spotify:"])
                            .spawn();
                    }
                } else {
                    win_impl::execute_media_action(&action_str);
                }
                std::thread::sleep(std::time::Duration::from_millis(160));
            }
            collect_music_snapshot_sync()
        })
        .await;

        if let Ok(snap) = updated {
            state_sig.set(snap);
        }
    });
}

pub fn spawn_music_monitor(mut music_ctx: MusicContext) {
    spawn(async move {
        loop {
            if let Ok(snap) = tokio::task::spawn_blocking(collect_music_snapshot_sync).await {
                if *music_ctx.state.read() != snap {
                    music_ctx.state.set(snap);
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(1400)).await;
        }
    });
}

pub fn format_mm_ss(secs: u64) -> String {
    let mins = secs / 60;
    let rem = secs % 60;
    format!("{mins:02}:{rem:02}")
}

// ============================================================================
// COMPONENTE DE BARRA LATERAL: REPRODUCTOR INTELIGENTE ESTILO SPOTIFY + FILA
// ============================================================================

#[derive(Clone, Copy, PartialEq)]
enum DrawerMode {
    Closed,
    Queue,
    Smart,
}

#[derive(Clone, Copy, PartialEq)]
enum QueueTab {
    UpNext,
    Recent,
}

#[component]
pub fn SidebarMusicWidget() -> Element {
    let mut music_ctx = use_music_context();
    let active_view = try_use_context::<Signal<crate::models::ActiveView>>();
    let snap = music_ctx.state.read().clone();

    let mut drawer_mode = use_signal(|| DrawerMode::Closed);
    let mut queue_tab = use_signal(|| QueueTab::UpNext);
    let mut queue_input = use_signal(String::new);

    // Estado de ventana del componente Spotify: Minimizar, Cerrar y Mover (Flotante / Anclado)
    let mut is_minimized = use_signal(|| false);
    let mut is_closed = use_signal(|| false);
    let mut is_floating = use_signal(|| false);
    let mut is_dragging = use_signal(|| false);
    let mut float_pos = use_signal(|| (275.0f64, 520.0f64));
    let mut drag_offset = use_signal(|| (110.0f64, 14.0f64));

    if !snap.detected {
        return rsx! {};
    }

    let app_name = snap.primary_app_name();
    let is_playing = snap.is_playing;

    let title = snap
        .current_track
        .as_ref()
        .map(|t| t.title.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{app_name} abierto"));

    let artist = snap
        .current_track
        .as_ref()
        .map(|t| t.artist.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if is_playing {
                "Reproduciendo en vivo".to_string()
            } else {
                "En pausa".to_string()
            }
        });

    let cover_uri = snap
        .current_track
        .as_ref()
        .and_then(|t| t.thumbnail_data_uri.clone());

    let pos_secs = snap
        .current_track
        .as_ref()
        .and_then(|t| t.position_secs)
        .unwrap_or(0);
    let dur_secs = snap
        .current_track
        .as_ref()
        .and_then(|t| t.duration_secs)
        .unwrap_or(0);

    let progress_pct = if dur_secs > 0 {
        ((pos_secs as f64 / dur_secs as f64) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    let is_shuffle = snap
        .current_track
        .as_ref()
        .map(|t| t.is_shuffle)
        .unwrap_or(false);
    let repeat_mode = snap
        .current_track
        .as_ref()
        .map(|t| t.repeat_mode.clone())
        .unwrap_or_else(|| "none".to_string());

    let up_next_count = snap.up_next.len();
    let cur_drawer = *drawer_mode.read();
    let cur_qtab = *queue_tab.read();

    let dot_class = if is_playing {
        "sidebar-music-dot is-playing animate-pulse"
    } else {
        "sidebar-music-dot"
    };

    let eyebrow_class = if is_playing {
        "sidebar-music-eyebrow is-playing"
    } else {
        "sidebar-music-eyebrow"
    };

    let card_class = if is_playing {
        "sidebar-music-card is-playing"
    } else {
        "sidebar-music-card"
    };

    let minimized_now = *is_minimized.read();
    let closed_now = *is_closed.read();
    let floating_now = *is_floating.read();
    let dragging_now = *is_dragging.read();
    let (fx, fy) = *float_pos.read();

    let section_class = if floating_now {
        "sidebar-music-section is-floating select-none"
    } else {
        "sidebar-music-section select-none"
    };

    let section_style = if floating_now {
        format!("left: {fx:.0}px; top: {fy:.0}px;")
    } else {
        String::new()
    };

    // Si el usuario cerró el componente, mostrar píldora compacta para reabrirlo
    if closed_now {
        return rsx! {
            div {
                class: "sidebar-music-section select-none",
                button {
                    type: "button",
                    class: "spotify-closed-restore",
                    title: "Restaurar reproductor de {app_name}",
                    onclick: move |_| is_closed.set(false),
                    div {
                        class: "flex items-center gap-1.5 min-w-0",
                        span { class: "{dot_class}" }
                        span { class: "truncate", "♫ Mostrar {app_name}: {title}" }
                    }
                    span { "+" }
                }
            }
        };
    }

    rsx! {
        // Capa global de captura durante el arrastre libre del reproductor Spotify
        if dragging_now {
            div {
                class: "spotify-drag-overlay",
                onmousemove: move |evt| {
                    let coords = evt.client_coordinates();
                    let (ox, oy) = *drag_offset.read();
                    let nx = (coords.x - ox).max(8.0);
                    let ny = (coords.y - oy).max(38.0);
                    float_pos.set((nx, ny));
                },
                onmouseup: move |_| {
                    is_dragging.set(false);
                },
            }
        }

        div {
            class: "{section_class}",
            style: "{section_style}",

            // Tarjeta Unificada con Glassmorphism Profesional (Modo Minimizado de 1 línea vs Modo Completo Compacto)
            if minimized_now {
                div {
                    class: if is_playing { "spotify-minimized-row is-playing" } else { "spotify-minimized-row" },
                    div {
                        class: "flex items-center justify-between gap-2 w-full",
                        div {
                            class: "flex items-center gap-2 min-w-0 flex-1",
                            div {
                                class: "spotify-mini-cover",
                                if let Some(ref uri) = cover_uri {
                                    img {
                                        class: "spotify-cover-img",
                                        src: "{uri}",
                                        alt: "{title}",
                                    }
                                } else {
                                    span { "♫" }
                                }
                            }
                            div {
                                class: "sidebar-music-info",
                                div {
                                    class: "sidebar-music-title",
                                    title: "{title} — {artist}",
                                    "{title}"
                                }
                            }
                        }

                        div {
                            class: "flex items-center gap-1.5 flex-none",
                            button {
                                type: "button",
                                class: "spotify-mini-play",
                                title: if is_playing { "Pausar" } else { "Reproducir" },
                                onclick: move |_| trigger_music_action("play_pause", music_ctx.state),
                                if is_playing {
                                    svg {
                                        class: "w-2.5 h-2.5 fill-current",
                                        view_box: "0 0 24 24",
                                        rect { x: "6", y: "5", width: "4", height: "14", rx: "1" }
                                        rect { x: "14", y: "5", width: "4", height: "14", rx: "1" }
                                    }
                                } else {
                                    svg {
                                        class: "w-2.5 h-2.5 fill-current",
                                        view_box: "0 0 24 24",
                                        polygon { points: "7 4 20 12 7 20 7 4" }
                                    }
                                }
                            }

                            // Button Group Compacto en Modo Minimizado
                            div {
                                class: "spotify-win-btngroup",
                                button {
                                    type: "button",
                                    class: if floating_now { "spotify-win-btn is-move is-active" } else { "spotify-win-btn is-move" },
                                    title: "Arrastra para mover o doble clic para anclar",
                                    onmousedown: move |evt| {
                                        evt.stop_propagation();
                                        let c = evt.client_coordinates();
                                        if !*is_floating.read() {
                                            is_floating.set(true);
                                            float_pos.set(((c.x - 160.0).max(12.0), (c.y - 14.0).max(40.0)));
                                            drag_offset.set((160.0, 14.0));
                                        } else {
                                            let (cur_x, cur_y) = *float_pos.read();
                                            drag_offset.set((c.x - cur_x, c.y - cur_y));
                                        }
                                        is_dragging.set(true);
                                    },
                                    ondoubleclick: move |evt| {
                                        evt.stop_propagation();
                                        is_dragging.set(false);
                                        is_floating.set(false);
                                    },
                                    svg {
                                        class: "w-2.5 h-2.5 fill-none stroke-current stroke-2",
                                        view_box: "0 0 24 24",
                                        path { d: "M5 9l-3 3 3 3M9 5l3-3 3 3M15 19l-3 3-3-3M19 9l3 3-3 3M2 12h20M12 2v20" }
                                    }
                                }
                                button {
                                    type: "button",
                                    class: "spotify-win-btn",
                                    title: "Expandir reproductor Spotify",
                                    onclick: move |_| is_minimized.set(false),
                                    svg {
                                        class: "w-2.5 h-2.5 fill-none stroke-current stroke-2",
                                        view_box: "0 0 24 24",
                                        rect { x: "4", y: "4", width: "16", height: "16", rx: "2" }
                                    }
                                }
                                button {
                                    type: "button",
                                    class: "spotify-win-btn is-close",
                                    title: "Cerrar reproductor Spotify",
                                    onclick: move |_| {
                                        is_dragging.set(false);
                                        is_closed.set(true);
                                    },
                                    svg {
                                        class: "w-2.5 h-2.5 fill-none stroke-current stroke-2",
                                        view_box: "0 0 24 24",
                                        line { x1: "18", y1: "6", x2: "6", y2: "18" }
                                        line { x1: "6", y1: "6", x2: "18", y2: "18" }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                // Tarjeta Reproductor Glassmorphic Estilo Spotify Compacta
                div {
                    class: "{card_class}",

                    // Nivel 1 Integrado: Fuente + Abrir + Button Group (Mover | Minimizar | Cerrar)
                    div {
                        class: "sidebar-music-header",
                        div {
                            class: "sidebar-music-source",
                            span { class: "{dot_class}" }
                            span {
                                class: "{eyebrow_class}",
                                "{app_name}"
                            }
                            if let Some(rem) = snap.sleep_timer_remaining_secs {
                                span {
                                    class: "spotify-queue-count",
                                    title: "Temporizador de apagado activo",
                                    "⏱ {format_mm_ss(rem)}"
                                }
                            }
                        }
                        div {
                            class: "sidebar-music-header-actions",
                            button {
                                type: "button",
                                class: "sidebar-music-open-btn",
                                title: "Traer ventana de {app_name} al frente",
                                onclick: move |_| trigger_music_action("focus_app", music_ctx.state),
                                span { "Abrir" }
                                svg {
                                    class: "w-2.5 h-2.5 fill-none stroke-current stroke-2",
                                    view_box: "0 0 24 24",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M7 17L17 7M7 7h10v10" }
                                }
                            }

                            // Button Group Glassmorphic: Mover, Minimizar y Cerrar
                            div {
                                class: "spotify-win-btngroup",
                                button {
                                    type: "button",
                                    class: if floating_now { "spotify-win-btn is-move is-active" } else { "spotify-win-btn is-move" },
                                    title: if floating_now {
                                        "Arrastra para mover o haz doble clic para acoplar de nuevo a la barra lateral"
                                    } else {
                                        "Mantén presionado y arrastra para mover libremente el reproductor"
                                    },
                                    onmousedown: move |evt| {
                                        evt.stop_propagation();
                                        let c = evt.client_coordinates();
                                        if !*is_floating.read() {
                                            is_floating.set(true);
                                            float_pos.set(((c.x - 170.0).max(12.0), (c.y - 14.0).max(40.0)));
                                            drag_offset.set((170.0, 14.0));
                                        } else {
                                            let (cur_x, cur_y) = *float_pos.read();
                                            drag_offset.set((c.x - cur_x, c.y - cur_y));
                                        }
                                        is_dragging.set(true);
                                    },
                                    ondoubleclick: move |evt| {
                                        evt.stop_propagation();
                                        is_dragging.set(false);
                                        is_floating.set(false);
                                    },
                                    svg {
                                        class: "w-2.5 h-2.5 fill-none stroke-current stroke-2",
                                        view_box: "0 0 24 24",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        path { d: "M5 9l-3 3 3 3M9 5l3-3 3 3M15 19l-3 3-3-3M19 9l3 3-3 3M2 12h20M12 2v20" }
                                    }
                                }

                                if floating_now {
                                    button {
                                        type: "button",
                                        class: "spotify-win-btn",
                                        title: "Anclar de nuevo en la barra lateral",
                                        onclick: move |_| {
                                            is_dragging.set(false);
                                            is_floating.set(false);
                                        },
                                        svg {
                                            class: "w-2.5 h-2.5 fill-none stroke-current stroke-2",
                                            view_box: "0 0 24 24",
                                            stroke_linecap: "round",
                                            stroke_linejoin: "round",
                                            path { d: "M19 12H5M12 19l-7-7 7-7" }
                                        }
                                    }
                                }

                                button {
                                    type: "button",
                                    class: "spotify-win-btn",
                                    title: "Minimizar reproductor Spotify",
                                    onclick: move |_| {
                                        is_minimized.set(true);
                                        drawer_mode.set(DrawerMode::Closed);
                                    },
                                    svg {
                                        class: "w-2.5 h-2.5 fill-none stroke-current stroke-2",
                                        view_box: "0 0 24 24",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        line { x1: "5", y1: "12", x2: "19", y2: "12" }
                                    }
                                }

                                button {
                                    type: "button",
                                    class: "spotify-win-btn is-close",
                                    title: "Cerrar reproductor Spotify",
                                    onclick: move |_| {
                                        is_dragging.set(false);
                                        is_closed.set(true);
                                    },
                                    svg {
                                        class: "w-2.5 h-2.5 fill-none stroke-current stroke-2",
                                        view_box: "0 0 24 24",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        line { x1: "18", y1: "6", x2: "6", y2: "18" }
                                        line { x1: "6", y1: "6", x2: "18", y2: "18" }
                                    }
                                }
                            }
                        }
                    }

                // Nivel 2: Carátula Real del Álbum + Título + Artista
                div {
                    class: "sidebar-music-main",
                    div {
                        class: "spotify-cover-wrap",
                        if let Some(ref uri) = cover_uri {
                            img {
                                class: "spotify-cover-img",
                                src: "{uri}",
                                alt: "{title}",
                            }
                        } else {
                            div {
                                class: "spotify-cover-fallback",
                                svg {
                                    class: "w-4 h-4 fill-none stroke-current stroke-2",
                                    view_box: "0 0 24 24",
                                    path { d: "M9 18V5l12-2v13" }
                                    circle { cx: "6", cy: "18", r: "3" }
                                    circle { cx: "18", cy: "16", r: "3" }
                                }
                            }
                        }
                        if is_playing {
                            div {
                                class: "spotify-cover-eq-badge",
                                div {
                                    class: "sidebar-eq-bars",
                                    span { class: "sidebar-eq-bar" }
                                    span { class: "sidebar-eq-bar" }
                                    span { class: "sidebar-eq-bar" }
                                }
                            }
                        }
                    }

                    div {
                        class: "sidebar-music-info",
                        div {
                            class: "sidebar-music-title",
                            title: "{title}",
                            "{title}"
                        }
                        div {
                            class: "sidebar-music-artist",
                            title: "{artist}",
                            "{artist}"
                        }
                    }
                }

                // Barra de Progreso Interactiva (si la sesión provee duración)
                if dur_secs > 0 {
                    div {
                        class: "spotify-progress-wrap",
                        div {
                            class: "spotify-progress-bar-hit",
                            title: "Haz clic para saltar de posición en la pista",
                            onclick: move |evt| {
                                // Salto proporcional estimado en un ancho de tarjeta de ~216px
                                let click_x = evt.element_coordinates().x.max(0.0);
                                let ratio = (click_x / 214.0).clamp(0.0, 1.0);
                                let target_s = ((dur_secs as f64) * ratio).round() as u64;
                                trigger_music_action(format!("seek:{target_s}"), music_ctx.state);
                            },
                            div {
                                class: "spotify-progress-track",
                                div {
                                    class: "spotify-progress-fill",
                                    style: "width: {progress_pct:.1}%;",
                                    span { class: "spotify-progress-thumb" }
                                }
                            }
                        }
                        div {
                            class: "spotify-progress-times",
                            span { "{format_mm_ss(pos_secs)}" }
                            span { "{format_mm_ss(dur_secs)}" }
                        }
                    }
                }

                // Nivel 3: Controles de Transporte Estilo Spotify (Shuffle | Prev | Play Circular | Next | Repeat)
                div {
                    class: "spotify-transport-row",

                    // Aleatorio (Shuffle)
                    button {
                        type: "button",
                        class: if is_shuffle { "spotify-icon-ctl is-active" } else { "spotify-icon-ctl" },
                        title: if is_shuffle { "Aleatorio activado" } else { "Activar aleatorio" },
                        onclick: move |_| trigger_music_action("toggle_shuffle", music_ctx.state),
                        svg {
                            class: "w-3.5 h-3.5 fill-none stroke-current stroke-2",
                            view_box: "0 0 24 24",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            polyline { points: "16 3 21 3 21 8" }
                            line { x1: "4", y1: "20", x2: "21", y2: "3" }
                            polyline { points: "21 16 21 21 16 21" }
                            line { x1: "15", y1: "15", x2: "21", y2: "21" }
                            line { x1: "4", y1: "4", x2: "9", y2: "9" }
                        }
                        if is_shuffle {
                            span { class: "spotify-active-dot" }
                        }
                    }

                    // Anterior
                    button {
                        type: "button",
                        class: "spotify-icon-ctl",
                        title: "Pista anterior",
                        onclick: move |_| trigger_music_action("previous", music_ctx.state),
                        svg {
                            class: "w-4 h-4 fill-current",
                            view_box: "0 0 24 24",
                            polygon { points: "19 20 9 12 19 4 19 20" }
                            rect { x: "5", y: "4", width: "2.2", height: "16", rx: "1" }
                        }
                    }

                    // Botón Circular Verde Spotify (Play / Pause)
                    button {
                        type: "button",
                        class: "spotify-play-circle",
                        title: if is_playing { "Pausar" } else { "Reproducir" },
                        onclick: move |_| trigger_music_action("play_pause", music_ctx.state),
                        if is_playing {
                            svg {
                                class: "w-4 h-4 fill-current",
                                view_box: "0 0 24 24",
                                rect { x: "6", y: "5", width: "4.2", height: "14", rx: "1" }
                                rect { x: "13.8", y: "5", width: "4.2", height: "14", rx: "1" }
                            }
                        } else {
                            svg {
                                class: "w-4 h-4 fill-current",
                                view_box: "0 0 24 24",
                                polygon { points: "7 4 20 12 7 20 7 4" }
                            }
                        }
                    }

                    // Siguiente
                    button {
                        type: "button",
                        class: "spotify-icon-ctl",
                        title: "Siguiente pista",
                        onclick: move |_| trigger_music_action("next", music_ctx.state),
                        svg {
                            class: "w-4 h-4 fill-current",
                            view_box: "0 0 24 24",
                            polygon { points: "5 4 15 12 5 20 5 4" }
                            rect { x: "16.8", y: "4", width: "2.2", height: "16", rx: "1" }
                        }
                    }

                    // Repetir (Repeat)
                    button {
                        type: "button",
                        class: if repeat_mode != "none" { "spotify-icon-ctl is-active" } else { "spotify-icon-ctl" },
                        title: match repeat_mode.as_str() {
                            "track" => "Repetir canción actual",
                            "list" => "Repetir lista",
                            _ => "Activar repetición",
                        },
                        onclick: move |_| trigger_music_action("cycle_repeat", music_ctx.state),
                        svg {
                            class: "w-3.5 h-3.5 fill-none stroke-current stroke-2",
                            view_box: "0 0 24 24",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            polyline { points: "17 1 21 5 17 9" }
                            path { d: "M3 11V9a4 4 0 0 1 4-4h14" }
                            polyline { points: "7 23 3 19 7 15" }
                            path { d: "M21 13v2a4 4 0 0 1-4 4H3" }
                        }
                        if repeat_mode != "none" {
                            span { class: "spotify-active-dot" }
                        }
                    }
                }

                // Barra de herramientas inferior: Fila de Reproducción | Funciones Smart IA | Volumen
                div {
                    class: "spotify-toolbar-row",
                    div {
                        class: "flex items-center gap-1",
                        // Botón Fila de Reproducción (Queue)
                        button {
                            type: "button",
                            class: if cur_drawer == DrawerMode::Queue { "spotify-pill-tab is-open" } else { "spotify-pill-tab" },
                            title: "Abrir Fila de Reproducción e Historial",
                            onclick: move |_| {
                                if *drawer_mode.read() == DrawerMode::Queue {
                                    drawer_mode.set(DrawerMode::Closed);
                                } else {
                                    drawer_mode.set(DrawerMode::Queue);
                                }
                            },
                            svg {
                                class: "w-3 h-3 fill-none stroke-current stroke-2",
                                view_box: "0 0 24 24",
                                line { x1: "8", y1: "6", x2: "21", y2: "6" }
                                line { x1: "8", y1: "12", x2: "21", y2: "12" }
                                line { x1: "8", y1: "18", x2: "21", y2: "18" }
                                line { x1: "3", y1: "6", x2: "3.01", y2: "6" }
                                line { x1: "3", y1: "12", x2: "3.01", y2: "12" }
                                line { x1: "3", y1: "18", x2: "3.01", y2: "18" }
                            }
                            span { "Fila" }
                            if up_next_count > 0 {
                                span { class: "spotify-queue-count", "{up_next_count}" }
                            }
                        }

                        // Botón Funciones Inteligentes (DJ Kyo / Sleep Timer)
                        button {
                            type: "button",
                            class: if cur_drawer == DrawerMode::Smart { "spotify-pill-tab is-open" } else { "spotify-pill-tab" },
                            title: "Funciones de reproductor musical inteligente (DJ Kyo, Letra, Temporizador)",
                            onclick: move |_| {
                                if *drawer_mode.read() == DrawerMode::Smart {
                                    drawer_mode.set(DrawerMode::Closed);
                                } else {
                                    drawer_mode.set(DrawerMode::Smart);
                                }
                            },
                            span { "✨ Smart" }
                        }
                    }

                    // Control rápido de volumen del sistema
                    div {
                        class: "spotify-vol-group",
                        button {
                            type: "button",
                            class: "spotify-vol-btn",
                            title: "Bajar volumen",
                            onclick: move |_| trigger_music_action("volume_down", music_ctx.state),
                            "−"
                        }
                        button {
                            type: "button",
                            class: "spotify-vol-btn",
                            title: "Silenciar / Reactivar audio",
                            onclick: move |_| trigger_music_action("volume_mute", music_ctx.state),
                            svg {
                                class: "w-3 h-3 fill-none stroke-current stroke-2",
                                view_box: "0 0 24 24",
                                polygon { points: "11 5 6 9 2 9 2 15 6 15 11 19 11 5" }
                                path { d: "M15.54 8.46a5 5 0 0 1 0 7.07" }
                            }
                        }
                        button {
                            type: "button",
                            class: "spotify-vol-btn",
                            title: "Subir volumen",
                            onclick: move |_| trigger_music_action("volume_up", music_ctx.state),
                            "+"
                        }
                    }
                }

                // ============================================================
                // CAJÓN DESPLEGABLE 1: FILA DE REPRODUCCIÓN (COLA + RECIENTES)
                // ============================================================
                if cur_drawer == DrawerMode::Queue {
                    div {
                        class: "spotify-drawer",

                        // Pestañas: A continuación vs Recientes
                        div {
                            class: "spotify-drawer-tabs",
                            button {
                                type: "button",
                                class: if cur_qtab == QueueTab::UpNext { "spotify-drawer-tab is-active" } else { "spotify-drawer-tab" },
                                onclick: move |_| queue_tab.set(QueueTab::UpNext),
                                "A continuación ({up_next_count})"
                            }
                            button {
                                type: "button",
                                class: if cur_qtab == QueueTab::Recent { "spotify-drawer-tab is-active" } else { "spotify-drawer-tab" },
                                onclick: move |_| queue_tab.set(QueueTab::Recent),
                                "Recientes ({snap.history.len()})"
                            }
                        }

                        if cur_qtab == QueueTab::UpNext {
                            // Buscador / agregador rápido a la fila
                            div {
                                class: "spotify-queue-input-row",
                                input {
                                    class: "spotify-queue-input",
                                    r#type: "text",
                                    placeholder: "Artista - Canción para tu fila...",
                                    value: "{queue_input.read()}",
                                    oninput: move |evt| queue_input.set(evt.value()),
                                    onkeydown: move |evt: KeyboardEvent| {
                                        if evt.key() == Key::Enter {
                                            let val = queue_input.read().trim().to_string();
                                            if !val.is_empty() {
                                                trigger_music_action(format!("queue_add:{val}"), music_ctx.state);
                                                queue_input.set(String::new());
                                            }
                                        }
                                    }
                                }
                                button {
                                    type: "button",
                                    class: "spotify-queue-add-btn",
                                    title: "Añadir a la fila de reproducción",
                                    onclick: move |_| {
                                        let val = queue_input.read().trim().to_string();
                                        if !val.is_empty() {
                                            trigger_music_action(format!("queue_add:{val}"), music_ctx.state);
                                            queue_input.set(String::new());
                                        }
                                    },
                                    "+ Fila"
                                }
                            }

                            div {
                                class: "spotify-queue-list",

                                // Sonando ahora
                                div {
                                    class: "spotify-queue-section-label",
                                    span { "Sonando ahora" }
                                }
                                div {
                                    class: "spotify-queue-item is-now",
                                    div {
                                        class: "spotify-queue-item-left",
                                        span { class: "spotify-queue-idx", "♫" }
                                        div {
                                            class: "spotify-queue-text",
                                            span { class: "spotify-queue-title", "{title}" }
                                            span { class: "spotify-queue-sub", "{artist}" }
                                        }
                                    }
                                }

                                // Siguientes en la fila
                                div {
                                    class: "spotify-queue-section-label",
                                    span { "Siguiente en tu fila" }
                                    if !snap.up_next.is_empty() {
                                        button {
                                            type: "button",
                                            class: "text-[9.5px] text-zinc-400 hover:text-white cursor-pointer",
                                            onclick: move |_| trigger_music_action("queue_clear", music_ctx.state),
                                            "Limpiar"
                                        }
                                    }
                                }

                                if snap.up_next.is_empty() {
                                    div {
                                        class: "text-[10.5px] text-zinc-400 py-1 px-1",
                                        "Tu fila está vacía. Añade temas arriba o usa una sugerencia rápida:"
                                    }
                                    if !artist.is_empty() && artist != "En pausa" {
                                        {
                                            let art_mix = format!("{artist} Mix");
                                            let art_top = format!("Mejores de {artist}");
                                            rsx! {
                                                div {
                                                    class: "spotify-timer-chips",
                                                    button {
                                                        type: "button",
                                                        class: "spotify-timer-chip",
                                                        onclick: move |_| {
                                                            trigger_music_action(format!("queue_add:{art_mix}"), music_ctx.state);
                                                        },
                                                        "+ {artist} Mix"
                                                    }
                                                    button {
                                                        type: "button",
                                                        class: "spotify-timer-chip",
                                                        onclick: move |_| {
                                                            trigger_music_action(format!("queue_add:{art_top}"), music_ctx.state);
                                                        },
                                                        "+ Top {artist}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    for (idx, item) in snap.up_next.iter().enumerate() {
                                        {
                                            let item_id = item.id.clone();
                                            let play_q = if item.artist.is_empty() || item.artist == "Spotify" {
                                                item.title.clone()
                                            } else {
                                                format!("{} {}", item.artist, item.title)
                                            };
                                            let rm_id = item.id.clone();
                                            rsx! {
                                                div {
                                                    key: "{item_id}",
                                                    class: "spotify-queue-item",
                                                    div {
                                                        class: "spotify-queue-item-left",
                                                        span { class: "spotify-queue-idx", "{idx + 1}" }
                                                        div {
                                                            class: "spotify-queue-text",
                                                            span { class: "spotify-queue-title", "{item.title}" }
                                                            span { class: "spotify-queue-sub", "{item.artist}" }
                                                        }
                                                    }
                                                    div {
                                                        class: "spotify-queue-actions",
                                                        button {
                                                            type: "button",
                                                            class: "spotify-queue-mini-btn",
                                                            title: "Buscar y reproducir en Spotify ahora",
                                                            onclick: move |_| {
                                                                trigger_music_action(format!("play_query:{play_q}"), music_ctx.state);
                                                            },
                                                            "▶"
                                                        }
                                                        button {
                                                            type: "button",
                                                            class: "spotify-queue-mini-btn",
                                                            title: "Quitar de la fila",
                                                            onclick: move |_| {
                                                                trigger_music_action(format!("queue_remove:{rm_id}"), music_ctx.state);
                                                            },
                                                            "×"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // Pestaña: Historial de canciones recientes de la sesión
                            div {
                                class: "spotify-queue-list",
                                if snap.history.is_empty() {
                                    div {
                                        class: "text-[10.5px] text-zinc-400 py-2 text-center",
                                        "Las canciones que reproduzcas aparecerán aquí automáticamente."
                                    }
                                } else {
                                    for (idx, item) in snap.history.iter().enumerate() {
                                        {
                                            let h_id = item.id.clone();
                                            let replay_q = if item.artist.is_empty() {
                                                item.title.clone()
                                            } else {
                                                format!("{} {}", item.artist, item.title)
                                            };
                                            let requeue_q = if item.artist.is_empty() {
                                                item.title.clone()
                                            } else {
                                                format!("{} - {}", item.artist, item.title)
                                            };
                                            rsx! {
                                                div {
                                                    key: "{h_id}",
                                                    class: "spotify-queue-item",
                                                    div {
                                                        class: "spotify-queue-item-left",
                                                        span { class: "spotify-queue-idx", "{idx + 1}" }
                                                        div {
                                                            class: "spotify-queue-text",
                                                            span { class: "spotify-queue-title", "{item.title}" }
                                                            span { class: "spotify-queue-sub", "{item.artist}" }
                                                        }
                                                    }
                                                    div {
                                                        class: "spotify-queue-actions",
                                                        button {
                                                            type: "button",
                                                            class: "spotify-queue-mini-btn",
                                                            title: "Añadir a la fila",
                                                            onclick: move |_| {
                                                                trigger_music_action(format!("queue_add:{requeue_q}"), music_ctx.state);
                                                            },
                                                            "+"
                                                        }
                                                        button {
                                                            type: "button",
                                                            class: "spotify-queue-mini-btn",
                                                            title: "Abrir en Spotify",
                                                            onclick: move |_| {
                                                                trigger_music_action(format!("play_query:{replay_q}"), music_ctx.state);
                                                            },
                                                            "▶"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ============================================================
                // CAJÓN DESPLEGABLE 2: FUNCIONES DE REPRODUCTOR INTELIGENTE
                // ============================================================
                if cur_drawer == DrawerMode::Smart {
                    div {
                        class: "spotify-drawer",

                        div {
                            class: "spotify-queue-section-label",
                            span { "Inteligencia Musical con Kyo" }
                        }

                        div {
                            class: "spotify-smart-grid",
                            {
                                let t_clone = title.clone();
                                let a_clone = artist.clone();
                                rsx! {
                                    button {
                                        type: "button",
                                        class: "spotify-smart-btn",
                                        onclick: move |_| {
                                            let prompt = format!(
                                                "Actúa como DJ Kyo: estoy escuchando \"{t_clone}\" de {a_clone}. Arma una fila de reproducción inteligente de 5 canciones similares que mantengan este mismo ritmo/atmósfera y dime por qué encajan."
                                            );
                                            music_ctx.pending_chat_prompt.set(Some(prompt));
                                            if let Some(mut av) = active_view {
                                                av.set(crate::models::ActiveView::Chat);
                                            }
                                        },
                                        span { "🎧 DJ Kyo: Armar fila inteligente (5 temas)" }
                                        span { class: "text-[10px] text-emerald-400", "IA →" }
                                    }
                                }
                            }
                            {
                                let t_clone = title.clone();
                                let a_clone = artist.clone();
                                rsx! {
                                    button {
                                        type: "button",
                                        class: "spotify-smart-btn",
                                        onclick: move |_| {
                                            let prompt = format!(
                                                "Analiza la canción \"{t_clone}\" de {a_clone}: ¿cuál es el significado de su letra, su estilo musical y datos curiosos de su producción?"
                                            );
                                            music_ctx.pending_chat_prompt.set(Some(prompt));
                                            if let Some(mut av) = active_view {
                                                av.set(crate::models::ActiveView::Chat);
                                            }
                                        },
                                        span { "📖 Significado, historia y análisis musical" }
                                        span { class: "text-[10px] text-emerald-400", "IA →" }
                                    }
                                }
                            }
                        }

                        // Temporizador de apagado automático (Sleep Timer)
                        div {
                            class: "spotify-queue-section-label",
                            span { "Temporizador de pausa automática" }
                            if let Some(rem) = snap.sleep_timer_remaining_secs {
                                span { class: "text-emerald-400", "{format_mm_ss(rem)}" }
                            }
                        }
                        div {
                            class: "spotify-timer-chips",
                            button {
                                type: "button",
                                class: if snap.sleep_timer_remaining_secs.is_none() { "spotify-timer-chip is-active" } else { "spotify-timer-chip" },
                                onclick: move |_| trigger_music_action("sleep_timer:0", music_ctx.state),
                                "Off"
                            }
                            for mins in [15u64, 30, 45, 60] {
                                button {
                                    key: "timer-{mins}",
                                    type: "button",
                                    class: "spotify-timer-chip",
                                    onclick: move |_| trigger_music_action(format!("sleep_timer:{mins}"), music_ctx.state),
                                    "{mins}m"
                                }
                            }
                        }
                    }
                }
            }
            }
        }
    }
}
