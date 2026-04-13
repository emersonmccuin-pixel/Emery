import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath } from 'url'
import path from 'path'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) {
            return undefined
          }

          if (id.includes('@xterm') || id.includes('xterm')) {
            return 'terminal-vendor'
          }

          if (id.includes('@tauri-apps')) {
            return 'tauri-vendor'
          }

          if (id.includes('@tanstack/react-virtual')) {
            return 'virtualizer-vendor'
          }

          if (id.includes('react-markdown') || id.includes('remark') || id.includes('micromark') || id.includes('mdast') || id.includes('unified') || id.includes('hast') || id.includes('vfile') || id.includes('unist') || id.includes('devlop') || id.includes('decode-named-character') || id.includes('character-entities')) {
            return 'markdown-vendor'
          }

          return 'vendor'
        },
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
  },
  preview: {
    port: 1420,
    strictPort: true,
  },
})
