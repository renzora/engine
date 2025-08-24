export default {
  id: 'babylon-native',
  name: 'Babylon Native',
  version: '1.0.0',
  description: 'True native performance with C++ runtime',
  category: 'babylon',
  backend: 'native',
  performance: 'maximum',
  compatibility: 'excellent',
  platform: ['tauri'],
  viewport: './BabylonNativeViewport.jsx',
  features: [
    'meshes',
    'materials',
    'lighting',
    'shadows',
    'physics',
    'animations',
    'native-performance'
  ],
  requirements: {
    cpp_runtime: true,
    native_graphics: ['directx', 'metal', 'vulkan']
  }
};