import { onMount, onCleanup } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { 
  IconSettings as SettingsIcon, IconMaximize, IconFolder
} from '@tabler/icons-solidjs';

import SettingsDropdownContent from '@/ui/display/SettingsDropdownContent.jsx';
import AssetLibrary from './AssetLibrary';



export default function EditorPage() {
  onMount(() => {
    // Initializing editor components
    const api = usePluginAPI();
    

    // Settings moved to toolbar helper instead of tab


    api.panel('assets', {
      title: 'Assets',
      component: AssetLibrary,
      icon: IconFolder,
      order: 10,
      defaultHeight: 300
    });

    // Settings helper with dropdown
    api.helper('settings-button', {
      title: 'Settings',
      icon: SettingsIcon,
      order: 30,
      hasDropdown: true,
      dropdownComponent: SettingsDropdownContent,
      dropdownWidth: 320
    });

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