# Gestión y Cambio Rápido de Iconos en Kyo

El avatar de Bloub está configurado como el icono oficial de Kyo en todas las plataformas y áreas del sistema operativo:
- **Bandeja del sistema (System Tray / "Mostrar iconos")**
- **Barra de tareas (Taskbar)**
- **Administrador de tareas de Windows (Task Manager)**
- **macOS (Dock y Menu Bar)**
- **Linux (Desktop / Dash)**
- **Android & iOS**
- **Frontend / WebView (Favicon)**

Dado que este avatar es temporal, este proyecto cuenta con una herramienta automatizada para reemplazar el icono por cualquier otro logotipo o imagen en **un solo paso**.

---

## Cómo Cambiar el Icono Rápidamente

### Opción 1: Proporcionando la ruta a tu nueva imagen (Recomendado)
Puedes pasar directamente tu archivo PNG o SVG (se recomienda formato cuadrado de al menos 512x512 o 1024x1024):

```bash
pnpm run icon ruta/a/tu-nuevo-logo.png
```
o con SVG:
```bash
pnpm run icon ruta/a/tu-nuevo-logo.svg
```

### Opción 2: Reemplazando el archivo fuente predeterminado
Coloca tu nuevo archivo en `src-tauri/icons/source/icon.png` (o `app-icon.png` en la raíz) y ejecuta:

```bash
pnpm run icon
```

### Opción 3: Restaurar o regenerar el avatar de Bloub
Si deseas volver en cualquier momento al avatar de Bloub:

```bash
pnpm run icon:bloub
```

---

## ¿Qué hace el script automáticamente?

Al ejecutar el comando, el script `scripts/update-icon.mjs` realiza:
1. **Generación de iconos para Windows**:
   - `src-tauri/icons/icon.ico` con todas las resoluciones embebidas (16x16, 24x24, 32x32, 48x48, 64x64, 256x256).
   - Todos los tamaños de tiles y store de Windows (`Square30x30`, `Square44x44`, `Square150x150`, etc.).
2. **Generación para macOS**:
   - `src-tauri/icons/icon.icns` para el Dock de macOS y alta densidad Retina.
3. **Generación para Linux / Escritorio**:
   - `32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `icon.png`.
4. **Generación para Móviles**:
   - Android: `mipmap-mdpi`, `hdpi`, `xhdpi`, `xxhdpi`, `xxxhdpi` (estándar y round).
   - iOS: resoluciones `AppIcon-*.png`.
5. **Sincronización Web**:
   - Actualiza `public/favicon.svg` o `public/favicon.ico` para la interfaz web.

---

## Visualizar los Cambios en el Ejecutable

1. **Modo Desarrollo**:
   ```bash
   pnpm tauri dev
   ```
2. **Compilar Ejecutable Final**:
   ```bash
   pnpm tauri build
   ```

> [!TIP]
> **Caché de iconos en Windows:**
> Windows Explorer almacena en caché los iconos de los ejecutables en `%LOCALAPPDATA%\IconCache.db`. Si compilas un nuevo ejecutable con el mismo nombre y Windows muestra el icono anterior, puedes refrescar la caché abriendo PowerShell y ejecutando:
> ```powershell
> ie4uinit.exe -show
> ```
> O reiniciando el proceso `explorer.exe` desde el Administrador de tareas.
