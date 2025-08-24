import { createPlugin } from '@/api/plugin';
import { Play } from '@/ui/icons';
import Animation from '@/pages/editor/Animation.jsx';

export default createPlugin({
  id: 'animation-plugin',
  name: 'Animation Plugin',
  version: '1.0.0',
  description: 'Timeline-based animation editor with keyframes',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[AnimationPlugin] Animation plugin initialized');
  },

  async onStart(api) {
    console.log('[AnimationPlugin] Registering animation panel...');

    // Register bottom panel tab
    api.panel('animation', {
      title: 'Animation',
      icon: Play,
      component: Animation,
      order: 4,
      defaultHeight: 400,
      plugin: 'animation-plugin'
    });

    console.log('[AnimationPlugin] Animation panel registered');
  }
});