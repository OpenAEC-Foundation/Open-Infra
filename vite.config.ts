import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';
import os from 'os';
import { copyFileSync, mkdirSync, existsSync } from 'fs';

/** Tauri appData dir of Open 2D Studio (com.open2dstudio.app) per platform. */
function appDataDir(): string | null {
  if (process.env.APPDATA) return path.join(process.env.APPDATA, 'com.open2dstudio.app');
  if (process.platform === 'darwin') return path.join(os.homedir(), 'Library', 'Application Support', 'com.open2dstudio.app');
  if (process.platform === 'linux') {
    const base = process.env.XDG_DATA_HOME || path.join(os.homedir(), '.local', 'share');
    return path.join(base, 'com.open2dstudio.app');
  }
  return null;
}

function autoDeploy() {
  const dir = appDataDir();
  // Opt-in: DEPLOY=1 npm run build installs into the desktop app's extension folder.
  if (!dir || !process.env.DEPLOY) return { name: 'auto-deploy' };
  const targetDir = path.join(dir, 'extensions', 'open-infra');
  return {
    name: 'auto-deploy',
    closeBundle() {
      if (!existsSync(targetDir)) mkdirSync(targetDir, { recursive: true });
      copyFileSync(path.resolve(__dirname, 'dist/main.js'), path.join(targetDir, 'main.js'));
      copyFileSync(path.resolve(__dirname, 'manifest.json'), path.join(targetDir, 'manifest.json'));
      console.log(`\n  ✓ Deployed to ${targetDir}\n`);
    },
  };
}

export default defineConfig({
  plugins: [react(), autoDeploy()],
  build: {
    lib: {
      entry: path.resolve(__dirname, 'src/index.tsx'),
      formats: ['cjs'],
      fileName: () => 'main.js',
    },
    rollupOptions: {
      // Provided by the host at runtime; proj4 is bundled.
      external: ['open-2d-studio', 'react', 'react/jsx-runtime', 'lucide-react'],
    },
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
    minify: false,
  },
});
