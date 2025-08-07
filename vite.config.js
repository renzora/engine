import { resolve } from 'node:path'
import { defineConfig } from 'vite'
import fs from 'fs'

import viteReact from '@vitejs/plugin-react-oxc'
import viteFastifyReact from '@fastify/react/plugin'
import tailwindcss from '@tailwindcss/vite'
import Inspect from 'vite-plugin-inspect'

export default defineConfig({
  root: resolve(import.meta.dirname, 'client'),

  plugins: [
    viteReact(),
    viteFastifyReact(),
    tailwindcss(),
    Inspect()
  ],
  
  server: {
    port: 3000,
    host: 'localhost',
    https: (() => {
      try {
        const keyPath = resolve(import.meta.dirname, 'localhost+2-key.pem')
        const certPath = resolve(import.meta.dirname, 'localhost+2.pem')
        
        if (fs.existsSync(keyPath) && fs.existsSync(certPath)) {
          return {
            key: fs.readFileSync(keyPath),
            cert: fs.readFileSync(certPath),
          }
        }
      } catch (error) {
        console.warn('HTTPS certificates not found, falling back to HTTP')
      }
      return false
    })()
  },
  resolve: {
    alias: {
      '@': resolve(import.meta.dirname, 'client')
    }
  },
  worker: {
    format: 'es',
    plugins: () => [viteReact()]
  },
  build: {
    outDir: resolve(import.meta.dirname, 'dist'),
    rollupOptions: {
      output: {
        assetFileNames: (assetInfo) => {
          if (assetInfo.name && assetInfo.name.includes('Worker')) {
            return 'assets/[name]-[hash][extname]'
          }
          return 'assets/[name]-[hash][extname]'
        },
        manualChunks: (id) => {
          if (id.includes('@babylonjs/core') || id.includes('node_modules/@babylonjs/core')) {
            return 'babylon-core';
          }
          if (id.includes('@babylonjs/inspector') || id.includes('node_modules/@babylonjs/inspector')) {
            return 'babylon-inspector';
          }
          if (id.includes('@babylonjs/') || id.includes('node_modules/@babylonjs/')) {
            return 'babylon-extensions';
          }
          
          if (id.includes('react') || id.includes('react-dom')) {
            return 'react-vendor';
          }
          if (id.includes('react-router') || id.includes('history')) {
            return 'react-router';
          }
          
          if (id.includes('node_modules')) {
            return 'vendor';
          }
          
          if (id.includes('client/plugins')) {
            return 'app-plugins';
          }
        },
        chunkFileNames: 'assets/[name]-[hash].js',
        entryFileNames: 'assets/[name]-[hash].js'
      }
    },
    chunkSizeWarningLimit: 800,
    minify: 'terser',
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true
      }
    },
    commonjsOptions: {
      include: [/node_modules/]
    }
  },
  ssr: {
    external: [
      'use-sync-external-store'
    ]
  },
  experimental: {
    enableNativePlugin: false
  }
})