import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Em desenvolvimento, o WebSocket e a /api vão para o servidor do Jaci na 8080.
export default defineConfig({
  plugins: [react()],
  server: { host: true, proxy: { '/ws': { target: 'ws://127.0.0.1:8080', ws: true }, '/api': 'http://127.0.0.1:8080' } },
  // O three.js sozinho passa de 600 kB; não é problema para um centro de controle.
  build: { chunkSizeWarningLimit: 1600 },
})
