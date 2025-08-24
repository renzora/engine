import { createPlugin } from '@/api/plugin';
import Terrain from '@/pages/editor/Terrain.jsx';

// Create terrain icon
const TerrainIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <path d="M3 20h18L15 8l-3 6-4-4-5 10z"/>
    <path d="M3 12h18"/>
    <circle cx="7" cy="8" r="1"/>
    <circle cx="12" cy="6" r="1"/>
    <circle cx="17" cy="10" r="1"/>
  </svg>
);

export default createPlugin({
  id: 'terrain-plugin',
  name: 'Terrain Plugin',
  version: '1.0.0',
  description: 'Terrain sculpting and painting tools for landscapes',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[TerrainPlugin] Terrain plugin initialized');
  },

  async onStart(api) {
    console.log('[TerrainPlugin] Registering terrain panel...');

    // Register bottom panel tab
    api.panel('terrain', {
      title: 'Terrain',
      icon: TerrainIcon,
      component: Terrain,
      order: 5,
      defaultHeight: 450,
      plugin: 'terrain-plugin'
    });

    console.log('[TerrainPlugin] Terrain panel registered');
  }
});