import { createPlugin } from '@/api/plugin';
import Particles from '@/pages/editor/Particles.jsx';

// Create particle icon
const ParticleIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <circle cx="12" cy="12" r="2"/>
    <circle cx="8" cy="8" r="1.5"/>
    <circle cx="16" cy="8" r="1.5"/>
    <circle cx="8" cy="16" r="1.5"/>
    <circle cx="16" cy="16" r="1.5"/>
    <circle cx="6" cy="12" r="1"/>
    <circle cx="18" cy="12" r="1"/>
    <circle cx="12" cy="6" r="1"/>
    <circle cx="12" cy="18" r="1"/>
  </svg>
);

export default createPlugin({
  id: 'particles-plugin',
  name: 'Particles Plugin',
  version: '1.0.0',
  description: 'Particle system editor for creating visual effects',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[ParticlesPlugin] Particles plugin initialized');
  },

  async onStart(api) {
    console.log('[ParticlesPlugin] Registering particles panel...');

    // Register bottom panel tab
    api.panel('particles', {
      title: 'Particles',
      icon: ParticleIcon,
      component: Particles,
      order: 2,
      defaultHeight: 400,
      plugin: 'particles-plugin'
    });

    console.log('[ParticlesPlugin] Particles panel registered');
  }
});