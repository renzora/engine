import { createPlugin } from '@/api/plugin';
import TeamChat from '@/pages/editor/TeamChat.jsx';

// Create chat icon
const ChatIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
    <path d="M9 10h6"/>
    <path d="M9 14h4"/>
  </svg>
);

export default createPlugin({
  id: 'teamchat-plugin',
  name: 'Team Chat Plugin',
  version: '1.0.0',
  description: 'Real-time team communication and collaboration',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[TeamChatPlugin] Team Chat plugin initialized');
  },

  async onStart(api) {
    console.log('[TeamChatPlugin] Registering team chat tab...');

    // Register property tab
    api.tab('teamchat', {
      title: 'Team Chat',
      icon: ChatIcon,
      component: TeamChat,
      order: 11,
      plugin: 'teamchat-plugin'
    });

    console.log('[TeamChatPlugin] Team Chat tab registered');
  }
});