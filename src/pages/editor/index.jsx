import { onMount, onCleanup, createSignal, createEffect, For, Show } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { 
  IconCamera, IconGrid3x3, IconSettings as SettingsIcon, IconMaximize, IconVideo, IconFolder, IconGridDots, IconClock, IconSun
} from '@tabler/icons-solidjs';
import ThemeSwitcher from '@/ui/ThemeSwitcher';

import Scene from './Scene.jsx';
import SettingsComponent from './Settings.jsx';
import AssetLibrary from './AssetLibrary';

import { scriptEditorStore, scriptEditorActions } from '../../layout/stores/ScriptEditorStore.js';
import { getCurrentProject } from '@/api/bridge/projects';
import { readFile, writeFile } from '@/api/bridge/files';


export default function EditorPage() {
  onMount(() => {
    // Initializing editor components
    const api = usePluginAPI();
    

    api.tab('settings', {
      title: 'Settings',
      component: SettingsComponent,
      icon: SettingsIcon,
      order: 999
    });


    api.panel('assets', {
      title: 'Assets',
      component: AssetLibrary,
      icon: IconFolder,
      order: 10,
      defaultHeight: 300
    });

    
    // Gizmo snapping is now handled by the combined grid plugin
    
    
    api.helper('fullscreen-button', {
      title: 'Toggle Fullscreen',
      icon: IconMaximize,
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

    // Editor components registered
    
    onCleanup(() => {
      // Cleaning up editor components
    });
  });

  return null;
}