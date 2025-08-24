import { onMount, onCleanup, createSignal, createEffect, For, Show } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { 
  Camera, Grid3x3, Settings as SettingsIcon, Maximize, Video, Folder, Code,
  Terminal, Play, Sun, Bookmark, Paperclip
} from '@/ui/icons';
import CameraDropdownContent from '@/ui/display/CameraDropdownContent.jsx';
import GridDropdownContent from '@/ui/display/GridDropdownContent.jsx';
import ThemeSwitcher from '@/ui/ThemeSwitcher';
import RendererSwitcher from '@/ui/RendererSwitcher';

import Scene from './Scene.jsx';
import SettingsComponent from './Settings.jsx';
import AssetLibrary from './AssetLibrary';

// Bottom panel components
import Console from './Console.jsx';
import Particles from './Particles.jsx';
import Physics from './Physics.jsx';
import Animation from './Animation.jsx';
import Terrain from './Terrain.jsx';
import Nodes from './Nodes.jsx';

// Right panel components
import Bookmarks from './Bookmarks.jsx';
import TeamChat from './TeamChat.jsx';
import Lighting from './Lighting.jsx';
import PostProcessing from './PostProcessing.jsx';

import { scriptEditorStore, scriptEditorActions } from '../../layout/stores/ScriptEditorStore.js';
import { getCurrentProject } from '@/api/bridge/projects';
import { readFile, writeFile } from '@/api/bridge/files';

// Custom icons for panels
const ParticleIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <circle cx="12" cy="12" r="2"/>
    <circle cx="8" cy="8" r="1.5"/>
    <circle cx="16" cy="8" r="1.5"/>
    <circle cx="8" cy="16" r="1.5"/>
    <circle cx="16" cy="16" r="1.5"/>
  </svg>
);

const PhysicsIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <path d="M12 2L2 7v10l10 5 10-5V7l-10-5z"/>
    <polyline points="2,7 12,12 22,7"/>
    <polyline points="12,22 12,12"/>
  </svg>
);

const TerrainIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <path d="M3 20h18L15 8l-3 6-4-4-5 10z"/>
    <circle cx="7" cy="8" r="1"/>
    <circle cx="17" cy="10" r="1"/>
  </svg>
);

const NodesIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <circle cx="5" cy="6" r="3"/>
    <circle cx="19" cy="18" r="3"/>
    <circle cx="12" cy="12" r="3"/>
    <path d="M8 6h5"/>
    <path d="M15 12h2"/>
  </svg>
);

const ChatIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
    <path d="M9 10h6"/>
    <path d="M9 14h4"/>
  </svg>
);

const PostProcessingIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <rect x="3" y="3" width="18" height="18" rx="2"/>
    <circle cx="12" cy="12" r="3"/>
    <path d="M12 1v6M12 17v6"/>
  </svg>
);

export default function EditorPage() {
  onMount(() => {
    console.log('[EditorPage] Initializing editor components...');
    const api = usePluginAPI();
    
    api.tab('scene', {
      title: 'Scene',
      component: Scene,
      icon: Video,
      order: 10
    });

    api.tab('settings', {
      title: 'Settings',
      component: SettingsComponent,
      icon: SettingsIcon,
      order: 20
    });


    api.panel('assets', {
      title: 'Assets',
      component: AssetLibrary,
      icon: Folder,
      order: 10,
      defaultHeight: 300
    });

    // Register bottom panel tabs
    api.panel('console', {
      title: 'Console',
      component: Console,
      icon: Terminal,
      order: 20,
      defaultHeight: 300
    });

    api.panel('particles', {
      title: 'Particles',
      component: Particles,
      icon: ParticleIcon,
      order: 30,
      defaultHeight: 400
    });

    api.panel('physics', {
      title: 'Physics',
      component: Physics,
      icon: PhysicsIcon,
      order: 40,
      defaultHeight: 350
    });

    api.panel('animation', {
      title: 'Animation',
      component: Animation,
      icon: Play,
      order: 50,
      defaultHeight: 400
    });

    api.panel('terrain', {
      title: 'Terrain',
      component: Terrain,
      icon: TerrainIcon,
      order: 60,
      defaultHeight: 450
    });

    api.panel('nodes', {
      title: 'Nodes',
      component: Nodes,
      icon: NodesIcon,
      order: 70,
      defaultHeight: 500
    });

    // Register right panel property tabs
    api.tab('bookmarks', {
      title: 'Bookmarks',
      component: Bookmarks,
      icon: Bookmark,
      order: 100
    });

    api.tab('teamchat', {
      title: 'Team Chat',
      component: TeamChat,
      icon: ChatIcon,
      order: 110
    });

    api.tab('lighting', {
      title: 'Lighting',
      component: Lighting,
      icon: Sun,
      order: 120
    });

    api.tab('postprocessing', {
      title: 'Post Processing',
      component: PostProcessing,
      icon: PostProcessingIcon,
      order: 130
    });
    
    api.button('camera-helper', {
      title: 'Camera Options',
      icon: Camera,
      section: 'right',
      order: 10,
      hasDropdown: true,
      dropdownComponent: CameraDropdownContent,
      dropdownWidth: 256
    });
    
    api.button('grid-helper', {
      title: 'Grid Options',
      icon: Grid3x3,
      section: 'right',
      order: 20,
      hasDropdown: true,
      dropdownComponent: GridDropdownContent,
      dropdownWidth: 256
    });
    
    api.button('renderer-switcher', {
      title: 'Renderer',
      section: 'right',
      order: 25,
      isCustomComponent: true,
      customComponent: RendererSwitcher
    });
    
    api.button('theme-switcher', {
      title: 'Theme',
      section: 'right',
      order: 30,
      isCustomComponent: true,
      customComponent: ThemeSwitcher
    });
    
    api.button('fullscreen-button', {
      title: 'Toggle Fullscreen',
      icon: Maximize,
      section: 'right',
      order: 40,
      onClick: () => {
        if (!document.fullscreenElement) {
          document.documentElement.requestFullscreen().catch(err => {
            console.error('[EditorPage] Error attempting to enable fullscreen:', err);
          });
        } else {
          document.exitFullscreen();
        }
      }
    });

    console.log('[EditorPage] Editor components registered');
    
    onCleanup(() => {
      console.log('[EditorPage] Cleaning up editor components...');
    });
  });

  return null;
}