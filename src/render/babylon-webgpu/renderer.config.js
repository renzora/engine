export default {
  id: 'babylon-webgpu',
  name: 'Babylon.js (WebGPU)',
  version: '8.22.2',
  description: 'High-performance WebGPU renderer',
  category: 'babylon',
  backend: 'webgpu',
  performance: 'high',
  compatibility: 'good',
  platform: ['web', 'tauri'],
  viewport: './WebGPUViewport.jsx',
  features: [
    'meshes',
    'materials',
    'lighting', 
    'shadows',
    'compute-shaders',
    'post-processing',
    'physics',
    'animations'
  ],
  requirements: {
    webgpu: true,
    browser: ['chrome', 'firefox', 'safari']
  }
};