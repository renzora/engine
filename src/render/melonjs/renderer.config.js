export default {
  id: 'melonjs',
  name: 'MelonJS',
  version: '17.4',
  description: '2D game engine with entity component system',
  category: '2d',
  backend: 'webgl/canvas',
  performance: 'high',
  compatibility: 'excellent',
  platform: ['web', 'tauri'],
  viewport: './MelonViewport.jsx',
  features: [
    '2d-graphics',
    'sprites',
    'animations',
    'physics',
    'entity-system',
    'tilemaps',
    'audio'
  ],
  requirements: {
    webgl: '1.0'
  }
};