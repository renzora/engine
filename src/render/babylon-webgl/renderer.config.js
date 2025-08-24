export default {
  id: 'babylon-webgl',
  name: 'Babylon.js (WebGL)',
  version: '8.22.2',
  description: 'Maximum compatibility WebGL renderer',
  category: 'babylon',
  backend: 'webgl',
  performance: 'standard',
  compatibility: 'excellent',
  platform: ['web', 'tauri'],
  viewport: './WebGLViewport.jsx',
  features: [
    'meshes',
    'materials', 
    'lighting',
    'shadows',
    'post-processing',
    'physics',
    'animations'
  ],
  requirements: {
    webgl: '2.0',
    extensions: ['OES_texture_float']
  }
};