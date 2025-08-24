export default {
  id: 'playcanvas',
  name: 'PlayCanvas',
  version: '1.70+',
  description: 'Enterprise WebGL engine with visual editor',
  category: 'webgl',
  backend: 'webgl',
  performance: 'high',
  compatibility: 'excellent',
  platform: ['web', 'tauri'],
  viewport: './PlayCanvasViewport.jsx',
  features: [
    'entity-component-system',
    'visual-editor',
    'physics',
    'audio',
    'networking',
    'enterprise-ready'
  ],
  requirements: {
    webgl: '1.0'
  }
};