import { createPlugin } from '@/api/plugin';
import PostProcessing from '@/pages/editor/PostProcessing.jsx';

// Create post processing icon
const PostProcessingIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <rect x="3" y="3" width="18" height="18" rx="2"/>
    <circle cx="12" cy="12" r="3"/>
    <path d="M12 1v6M12 17v6M4.22 4.22l4.24 4.24M15.54 15.54l4.24 4.24M1 12h6M17 12h6M4.22 19.78l4.24-4.24M15.54 8.46l4.24-4.24"/>
  </svg>
);

export default createPlugin({
  id: 'postprocessing-plugin',
  name: 'Post Processing Plugin',
  version: '1.0.0',
  description: 'Visual effects pipeline with bloom, DOF, and color grading',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[PostProcessingPlugin] Post Processing plugin initialized');
  },

  async onStart(api) {
    console.log('[PostProcessingPlugin] Registering post processing tab...');

    // Register property tab
    api.tab('postprocessing', {
      title: 'Post Processing',
      icon: PostProcessingIcon,
      component: PostProcessing,
      order: 13,
      plugin: 'postprocessing-plugin'
    });

    console.log('[PostProcessingPlugin] Post Processing tab registered');
  }
});