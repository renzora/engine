export default {
  id: 'custom-vulkan',
  name: 'Torus R1',
  version: '0.1.0',
  description: 'Torus R1 - Custom Vulkan renderer for graphics library development',
  category: 'custom',
  backend: 'vulkan',
  performance: 'maximum',
  compatibility: 'experimental',
  platform: ['tauri'],
  viewport: './VulkanViewport.jsx',
  features: [
    'low-level-control',
    'custom-shaders',
    'experimental',
    'learning-platform'
  ],
  requirements: {
    vulkan: '1.0',
    native_only: true
  }
};