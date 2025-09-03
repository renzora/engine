import { onMount, onCleanup, createSignal, createEffect, For, Show } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { 
  Camera, Grid3x3, Settings as SettingsIcon, Maximize, Video, Folder, Grid, Clock, Sun
} from '@/ui/icons';
import GizmoDropdownContent from '@/ui/display/GizmoDropdownContent.jsx';
import ThemeSwitcher from '@/ui/ThemeSwitcher';

import Scene from './Scene.jsx';
import SettingsComponent from './Settings.jsx';
import AssetLibrary from './AssetLibrary';

import { scriptEditorStore, scriptEditorActions } from '../../layout/stores/ScriptEditorStore.js';
import { getCurrentProject } from '@/api/bridge/projects';
import { readFile, writeFile } from '@/api/bridge/files';


export default function EditorPage() {
  onMount(() => {
    console.log('[EditorPage] Initializing editor components...');
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
      icon: Folder,
      order: 10,
      defaultHeight: 300
    });

    
    api.helper('gizmo-helper', {
      title: 'Gizmo Options',
      icon: Grid,
      order: 5,
      hasDropdown: true,
      dropdownComponent: GizmoDropdownContent,
      dropdownWidth: 224
    });
    
    
    
    
    
    api.helper('fullscreen-button', {
      title: 'Toggle Fullscreen',
      icon: Maximize,
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