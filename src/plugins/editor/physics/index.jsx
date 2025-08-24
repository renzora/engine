import { createPlugin } from '@/api/plugin';
import Physics from '@/pages/editor/Physics.jsx';

// Create physics icon
const PhysicsIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <path d="M12 2L2 7v10l10 5 10-5V7l-10-5z"/>
    <polyline points="2,7 12,12 22,7"/>
    <polyline points="12,22 12,12"/>
    <circle cx="8" cy="10" r="1"/>
    <circle cx="16" cy="10" r="1"/>
    <path d="M8 14l8-2"/>
  </svg>
);

export default createPlugin({
  id: 'physics-plugin',
  name: 'Physics Plugin',
  version: '1.0.0',
  description: 'Physics simulation and rigid body dynamics editor',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[PhysicsPlugin] Physics plugin initialized');
  },

  async onStart(api) {
    console.log('[PhysicsPlugin] Registering physics panel...');

    // Register bottom panel tab
    api.panel('physics', {
      title: 'Physics',
      icon: PhysicsIcon,
      component: Physics,
      order: 3,
      defaultHeight: 350,
      plugin: 'physics-plugin'
    });

    console.log('[PhysicsPlugin] Physics panel registered');
  }
});