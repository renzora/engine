export default {
  id: 'pixijs',
  name: 'PixiJS',
  version: '8.x',
  description: '2D WebGL renderer optimized for performance',
  category: '2d',
  backend: 'webgl',
  performance: 'maximum',
  compatibility: 'excellent',
  platform: ['web', 'tauri'],
  viewport: './PixiViewport.jsx',
  features: [
    '2d-graphics',
    'sprites',
    'filters',
    'particles',
    'high-performance',
    'mobile-optimized'
  ],
  requirements: {
    webgl: '1.0'
  }
};