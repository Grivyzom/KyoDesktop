#!/usr/bin/env node

/**
 * Script de actualización y regeneración de iconos para Kyo (Tauri v2)
 *
 * Soporta cualquier imagen (PNG, SVG) y genera automáticamente:
 * - Windows: icon.ico (16, 24, 32, 48, 64, 256), Square*.png, StoreLogo.png
 * - macOS: icon.icns
 * - Linux / Desktop: 32x32.png, 64x64.png, 128x128.png, 128x128@2x.png, icon.png
 * - Android: mipmap-* (ic_launcher, round, foreground)
 * - iOS: AppIcon-*
 * - Web / WebView: public/favicon.svg
 *
 * Uso:
 *   pnpm run icon                     # Usa el archivo fuente por defecto (o Bloub)
 *   pnpm run icon <ruta-imagen>       # Usa la imagen indicada (ej: pnpm run icon nuevo-logo.png)
 *   pnpm run icon:bloub               # Regenera o restaura el avatar de Bloub
 */

import { existsSync, copyFileSync, readFileSync } from 'fs';
import { resolve, dirname, extname } from 'path';
import { fileURLToPath } from 'url';
import { execSync } from 'child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootDir = resolve(__dirname, '..');
const iconsDir = resolve(rootDir, 'src-tauri/icons');
const sourceDir = resolve(iconsDir, 'source');
const bloubSvg = resolve(sourceDir, 'bloub-avatar.svg');

// 1. Determinar el archivo de entrada
const args = process.argv.slice(2);
let inputPath = null;

if (args.includes('--bloub') || args.includes('-b')) {
  inputPath = bloubSvg;
} else if (args[0] && !args[0].startsWith('-')) {
  inputPath = resolve(process.cwd(), args[0]);
} else {
  // Búsqueda de archivo fuente por defecto
  const candidates = [
    resolve(sourceDir, 'current-icon.png'),
    resolve(sourceDir, 'current-icon.svg'),
    resolve(sourceDir, 'icon.png'),
    resolve(sourceDir, 'icon.svg'),
    resolve(rootDir, 'app-icon.png'),
    resolve(rootDir, 'app-icon.svg'),
    bloubSvg
  ];

  for (const cand of candidates) {
    if (existsSync(cand)) {
      inputPath = cand;
      break;
    }
  }
}

if (!inputPath || !existsSync(inputPath)) {
  console.error(`\x1b[31m[ERROR] No se encontró el archivo de imagen de origen: ${inputPath || '(ninguno especificado)'}\x1b[0m`);
  console.error(`Uso: pnpm run icon <ruta/a/imagen.[png|svg]>`);
  console.error(`O coloca tu imagen en: src-tauri/icons/source/icon.png`);
  process.exit(1);
}

console.log(`\x1b[36m[Kyo Icon Tool]\x1b[0m Procesando icono desde: \x1b[33m${inputPath}\x1b[0m`);

try {
  // 2. Ejecutar Tauri Icon CLI
  console.log(`\x1b[34m→\x1b[0m Generando iconos multiplataforma con Tauri CLI...`);
  const tauriCmd = `npx @tauri-apps/cli icon "${inputPath}" -o "${iconsDir}"`;
  execSync(tauriCmd, { cwd: rootDir, stdio: 'inherit' });

  // 3. Sincronizar icono web (public/favicon.svg o public/favicon.ico)
  const ext = extname(inputPath).toLowerCase();
  const publicDir = resolve(rootDir, 'public');

  if (ext === '.svg') {
    copyFileSync(inputPath, resolve(publicDir, 'favicon.svg'));
    console.log(`\x1b[32m✓\x1b[0m Sincronizado public/favicon.svg`);
  } else if (existsSync(resolve(iconsDir, 'icon.ico'))) {
    copyFileSync(resolve(iconsDir, 'icon.ico'), resolve(publicDir, 'favicon.ico'));
    console.log(`\x1b[32m✓\x1b[0m Sincronizado public/favicon.ico`);
  }

  // Si no es el bloub predeterminado, guardamos una copia de respaldo en source/
  if (inputPath !== bloubSvg) {
    const backupTarget = resolve(sourceDir, `current-icon${ext}`);
    copyFileSync(inputPath, backupTarget);
    console.log(`\x1b[32m✓\x1b[0m Copia de origen guardada en: src-tauri/icons/source/current-icon${ext}`);
  }

  console.log(`\n\x1b[32m🎉 ¡Todos los iconos fueron generados exitosamente!\x1b[0m`);
  console.log(`\x1b[90mPlataformas cubiertas: Windows (.ico + Store tiles), macOS (.icns), Linux (.png), Android (mipmaps), iOS (AppIcons), Tray y Web.\x1b[0m\n`);
  console.log(`Para probar el nuevo icono en la app y el Administrador de tareas:`);
  console.log(`  \x1b[33mpnpm tauri dev\x1b[0m   (Modo desarrollo)`);
  console.log(`  \x1b[33mpnpm tauri build\x1b[0m (Compilación final de producción)\n`);
} catch (error) {
  console.error(`\x1b[31m[ERROR] Falló la generación de iconos:\x1b[0m`, error.message);
  process.exit(1);
}
