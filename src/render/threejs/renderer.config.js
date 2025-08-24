export default {
  id: 'threejs',
  name: 'Three.js',
  version: '0.160+',
  description: 'Popular WebGL library with extensive features',
  category: 'webgl',
  backend: 'webgl',
  performance: 'high',
  compatibility: 'excellent',
  platform: ['web', 'tauri'],
  viewport: './ThreeViewport.jsx',
  features: [
    'meshes',
    'materials',
    'lighting',
    'shadows',
    'animations',
    'physics-ready',
    'large-ecosystem'
  ],
  requirements: {
    webgl: '1.0'
  }
};