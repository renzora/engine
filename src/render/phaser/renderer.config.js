export default {
  id: 'phaser',
  name: 'Phaser',
  version: '3.90',
  description: '2D HTML5 game framework with WebGL and Canvas support',
  category: '2d',
  backend: 'webgl/canvas',
  performance: 'high',
  compatibility: 'excellent',
  platform: ['web', 'tauri'],
  viewport: './PhaserViewport.jsx',
  features: [
    '2d-graphics',
    'sprites',
    'animations',
    'physics',
    'audio',
    'input-handling',
    'game-objects'
  ],
  requirements: {
    webgl: '1.0'
  }
};