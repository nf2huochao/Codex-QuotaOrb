import { defineConfig } from 'vite'

// The standalone web preview is served from Vite, while the live snapshot
// and pairing endpoints remain on the desktop LAN server.
export default defineConfig({
  clearScreen: false,
  server: {
    proxy: {
      '/api': { target: 'http://127.0.0.1:18765', changeOrigin: true },
      '/ws': { target: 'ws://127.0.0.1:18765', ws: true, changeOrigin: true },
    },
  },
})
