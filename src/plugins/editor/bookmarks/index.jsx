import { createPlugin } from '@/api/plugin';
import { Bookmark } from '@/ui/icons';
import Bookmarks from '@/pages/editor/Bookmarks.jsx';

export default createPlugin({
  id: 'bookmarks-plugin',
  name: 'Bookmarks Plugin',
  version: '1.0.0',
  description: 'Save and manage camera positions and scene states',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[BookmarksPlugin] Bookmarks plugin initialized');
  },

  async onStart(api) {
    console.log('[BookmarksPlugin] Registering bookmarks tab...');

    // Register property tab
    api.tab('bookmarks', {
      title: 'Bookmarks',
      icon: Bookmark,
      component: Bookmarks,
      order: 10,
      plugin: 'bookmarks-plugin'
    });

    console.log('[BookmarksPlugin] Bookmarks tab registered');
  }
});