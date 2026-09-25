#[cfg(windows)]
mod win32 {
    use std::collections::BTreeSet;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;

    type HKEY = isize;
    type LONG = i32;
    type DWORD = u32;
    type LSTATUS = LONG;

    const HKEY_CURRENT_USER: HKEY = 0x80000001_u32 as isize;
    const HKEY_LOCAL_MACHINE: HKEY = 0x80000002_u32 as isize;
    const KEY_READ: DWORD = 0x20019;
    const KEY_SET_VALUE: DWORD = 0x0002;
    const KEY_WOW64_64KEY: DWORD = 0x0100;
    const REG_SZ: DWORD = 1;
    const ERROR_SUCCESS: LSTATUS = 0;
    const ERROR_NO_MORE_ITEMS: LSTATUS = 259;

    const WM_FONTCHANGE: u32 = 0x001D;
    const HWND_BROADCAST: isize = 0xffff;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;

    extern "system" {
        fn RegOpenKeyExW(
            hKey: HKEY,
            lpSubKey: *const u16,
            ulOptions: DWORD,
            samDesired: DWORD,
            phkResult: *mut HKEY,
        ) -> LSTATUS;

        fn RegCreateKeyExW(
            hKey: HKEY,
            lpSubKey: *const u16,
            Reserved: DWORD,
            lpClass: *mut u16,
            dwOptions: DWORD,
            samDesired: DWORD,
            lpSecurityAttributes: *mut std::ffi::c_void,
            phkResult: *mut HKEY,
            lpdwDisposition: *mut DWORD,
        ) -> LSTATUS;

        fn RegEnumValueW(
            hKey: HKEY,
            dwIndex: DWORD,
            lpValueName: *mut u16,
            lpcchValueName: *mut DWORD,
            lpReserved: *mut DWORD,
            lpType: *mut DWORD,
            lpData: *mut u8,
            lpcbData: *mut DWORD,
        ) -> LSTATUS;

        fn RegSetValueExW(
            hKey: HKEY,
            lpValueName: *const u16,
            Reserved: DWORD,
            dwType: DWORD,
            lpData: *const u8,
            cbData: DWORD,
        ) -> LSTATUS;

        fn RegCloseKey(hKey: HKEY) -> LSTATUS;

        fn AddFontResourceExW(
            name: *const u16,
            fl: DWORD,
            res: *mut std::ffi::c_void,
        ) -> i32;

        fn SendMessageTimeoutW(
            hWnd: isize,
            Msg: u32,
            wParam: usize,
            lParam: isize,
            fuFlags: u32,
            uTimeout: u32,
            lpdwResult: *mut usize,
        ) -> isize;
    }

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Lee las fuentes registradas en una clave de registro específica (HKLM o HKCU)
    unsafe fn read_fonts_from_key(root: HKEY, subkey: &str, set: &mut BTreeSet<String>) {
        let wide_subkey = to_wide(subkey);
        let mut hk: HKEY = 0;

        // Abrir siempre con KEY_WOW64_64KEY para ver la vista de registro nativa de 64 bits
        if RegOpenKeyExW(root, wide_subkey.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &mut hk) != ERROR_SUCCESS {
            return;
        }

        let mut index = 0;
        let mut name_buf = [0u16; 512];

        loop {
            let mut name_len = name_buf.len() as DWORD;
            let status = RegEnumValueW(
                hk,
                index,
                name_buf.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );

            if status == ERROR_NO_MORE_ITEMS {
                break;
            }

            if status == ERROR_SUCCESS {
                let raw_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                // Limpiar sufijos técnicos de Windows ignorando mayúsculas/minúsculas y caracteres nulos
                let cleaned = clean_font_name(&raw_name);
                if !cleaned.is_empty() && !cleaned.starts_with('@') {
                    set.insert(cleaned);
                }
            }

            index += 1;
        }

        RegCloseKey(hk);
    }

    fn clean_font_name(raw: &str) -> String {
        let mut cleaned = raw.trim_matches('\0').trim();
        let lower = cleaned.to_lowercase();
        let suffixes = [
            " (truetype)",
            " (opentype)",
            " (all type)",
            " (truetype & opentype)",
            " (postscript)",
        ];
        for s in suffixes {
            if lower.ends_with(s) {
                cleaned = &cleaned[..cleaned.len() - s.len()];
                break;
            }
        }
        cleaned.trim().to_string()
    }

    /// Obtiene la lista completa y ordenada de tipografías instaladas en Windows
    pub fn get_system_fonts() -> Vec<String> {
        let mut set = BTreeSet::new();

        unsafe {
            // Fuentes del sistema a nivel máquina
            read_fonts_from_key(
                HKEY_LOCAL_MACHINE,
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts",
                &mut set,
            );
            // Fuentes instaladas por el usuario actual
            read_fonts_from_key(
                HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows NT\CurrentVersion\Fonts",
                &mut set,
            );
        }

        let mut list: Vec<String> = set.into_iter().collect();
        list.sort_by_key(|a| a.to_lowercase());
        list
    }

    /// Instala silenciosamente la fuente empaquetada en %LOCALAPPDATA%\Microsoft\Windows\Fonts
    /// y la registra en el registro de Windows HKCU sin requerir privilegios de Administrador.
    pub fn install_bundled_font(
        font_family_name: &str,
        file_name: &str,
        font_bytes: &[u8],
    ) -> Result<String, String> {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map_err(|e| format!("No se pudo obtener LOCALAPPDATA: {e}"))?;

        let fonts_dir = PathBuf::from(&local_app_data)
            .join("Microsoft")
            .join("Windows")
            .join("Fonts");

        std::fs::create_dir_all(&fonts_dir)
            .map_err(|e| format!("Error creando carpeta de fuentes de usuario: {e}"))?;

        let target_file = fonts_dir.join(file_name);

        // Verificar si el archivo ya existe y su tamaño coincide
        let need_write = match std::fs::metadata(&target_file) {
            Ok(meta) => meta.len() != font_bytes.len() as u64,
            Err(_) => true,
        };

        if need_write {
            // Si el archivo estuviera bloqueado por el caché de fuentes de Windows, no fallar catastróficamente
            if let Err(e) = std::fs::write(&target_file, font_bytes) {
                if !target_file.is_file() {
                    return Err(format!("Error escribiendo archivo de fuente {}: {e}", target_file.display()));
                }
            }
        }

        // Registrar la fuente en HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts
        let reg_value_name = format!("{} (TrueType)", font_family_name);
        let reg_value_data = target_file.to_string_lossy().to_string();

        unsafe {
            let wide_subkey = to_wide(r"Software\Microsoft\Windows NT\CurrentVersion\Fonts");
            let mut hk: HKEY = 0;
            let mut disposition: DWORD = 0;

            // Usar RegCreateKeyExW para crear la clave si no existiera en cuentas de usuario limpias
            let status = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide_subkey.as_ptr(),
                0,
                std::ptr::null_mut(),
                0,
                KEY_SET_VALUE | KEY_WOW64_64KEY,
                std::ptr::null_mut(),
                &mut hk,
                &mut disposition,
            );

            if status != ERROR_SUCCESS {
                return Err(format!("Error abriendo/creando clave de registro de fuentes HKCU: código {status}"));
            }

            let wide_val_name = to_wide(&reg_value_name);
            let wide_val_data = to_wide(&reg_value_data);
            let bytes_data = std::slice::from_raw_parts(
                wide_val_data.as_ptr() as *const u8,
                wide_val_data.len() * 2,
            );

            let set_status = RegSetValueExW(
                hk,
                wide_val_name.as_ptr(),
                0,
                REG_SZ,
                bytes_data.as_ptr(),
                bytes_data.len() as DWORD,
            );

            RegCloseKey(hk);

            if set_status != ERROR_SUCCESS {
                return Err(format!("Error escribiendo valor en registro de fuentes: código {set_status}"));
            }

            // Notificar a GDI y al subsistema de ventanas de Windows
            let wide_target = to_wide(&reg_value_data);
            AddFontResourceExW(wide_target.as_ptr(), 0, std::ptr::null_mut());

            let mut result: usize = 0;
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_FONTCHANGE,
                0,
                0,
                SMTO_ABORTIFHUNG,
                1000,
                &mut result,
            );
        }

        Ok(format!("Fuente '{}' instalada correctamente en {}", font_family_name, target_file.display()))
    }
}

#[cfg(not(windows))]
mod non_windows {
    pub fn get_system_fonts() -> Vec<String> {
        vec![
            "Segoe UI".to_string(),
            "Inter".to_string(),
            "Roboto".to_string(),
            "Arial".to_string(),
            "Helvetica".to_string(),
            "system-ui".to_string(),
        ]
    }

    pub fn install_bundled_font(
        _font_family_name: &str,
        _file_name: &str,
        _font_bytes: &[u8],
    ) -> Result<String, String> {
        Ok("Instalación de fuentes nativa solo requerida en Windows".to_string())
    }
}

// Byte data de la fuente empaquetada (Geist / Kyo Font)
const BUNDLED_FONT_BYTES: &[u8] = include_bytes!("../../src/assets/fonts/Kyo-Regular.ttf");
pub const BUNDLED_FONT_NAME: &str = "Kyo";
pub const BUNDLED_FONT_FILE: &str = "Kyo-Regular.ttf";

#[tauri::command]
pub fn get_system_fonts() -> Vec<String> {
    #[cfg(windows)]
    return win32::get_system_fonts();

    #[cfg(not(windows))]
    return non_windows::get_system_fonts();
}

#[tauri::command]
pub fn install_bundled_font() -> Result<String, String> {
    #[cfg(windows)]
    return win32::install_bundled_font(BUNDLED_FONT_NAME, BUNDLED_FONT_FILE, BUNDLED_FONT_BYTES);

    #[cfg(not(windows))]
    return non_windows::install_bundled_font(BUNDLED_FONT_NAME, BUNDLED_FONT_FILE, BUNDLED_FONT_BYTES);
}

#[tauri::command]
pub fn save_font_preference(font: String) -> Result<(), String> {
    let trimmed = font.trim().to_string();
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let kyo_dir = std::path::PathBuf::from(appdata).join("kyo");
        let _ = std::fs::create_dir_all(&kyo_dir);
        let settings_file = kyo_dir.join("settings.json");
        let mut json = if settings_file.is_file() {
            std::fs::read_to_string(&settings_file)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        json["font_family"] = serde_json::json!(trimmed);
        std::fs::write(&settings_file, json.to_string())
            .map_err(|e| format!("Error guardando settings.json: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_font_preference() -> String {
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let settings_file = std::path::PathBuf::from(appdata).join("kyo").join("settings.json");
        if settings_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&settings_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(f) = json.get("font_family").and_then(|t| t.as_str()) {
                        let trimmed = f.trim();
                        if !trimmed.is_empty() {
                            return trimmed.to_string();
                        }
                    }
                }
            }
        }
    }
    "Kyo".to_string()
}
