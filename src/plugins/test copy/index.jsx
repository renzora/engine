import { Plugin } from '@/plugins/core/engine/Plugin.jsx';
import { 
  IconTestPipe, 
  IconBulb,
  IconTerminal,
  IconWand,
  IconChevronDown,
  IconPalette,
  IconCode,
  IconSettings
} from '@tabler/icons-solidjs';
import { createSignal } from 'solid-js';

// Test Viewport Component
function TestViewport() {
  return (
    <div class="w-full h-full bg-gray-900 text-white p-4">
      <h1 class="text-xl font-bold mb-4">Test Viewport</h1>
      <p class="text-gray-400">This is a simple viewport from the test plugin.</p>
    </div>
  );
}

// Test Properties Panel Component
function TestPropertiesPanel() {
  return (
    <div class="p-4">
      <h3 class="font-semibold text-white mb-2">Test Properties</h3>
      <p class="text-sm text-gray-400">This is a property panel from the test plugin.</p>
    </div>
  );
}

// Test Console Component  
function TestConsole() {
  return (
    <div class="h-full bg-gray-900 p-4">
      <h3 class="font-semibold text-white mb-2">Test Console</h3>
      <p class="text-sm text-gray-400">This is a console panel from the test plugin.</p>
    </div>
  );
}

// Test Dropdown Content Component (only the dropdown content)
function TestDropdownContent() {
  const [selectedAction, setSelectedAction] = createSignal('None');
  
  const dropdownActions = [
    { id: 'design', label: 'Design Mode', icon: IconPalette },
    { id: 'code', label: 'Code Mode', icon: IconCode },
    { id: 'settings', label: 'Test Settings', icon: IconSettings }
  ];
  
  const handleActionSelect = (action) => {
    setSelectedAction(action.label);
    console.log(`[TestPlugin] Dropdown action selected: ${action.label}`);
  };
  
  return (
    <div class="w-48 space-y-1 p-2">
      <div class="px-2 py-1 text-gray-400 border-b border-gray-700 mb-1">
        Test Plugin Actions
      </div>
      <div class="mb-2 px-2 py-1 text-xs text-gray-500">
        Selected: {selectedAction()}
      </div>
      
      {dropdownActions.map((action) => (
        <button
          onClick={() => handleActionSelect(action)}
          class="w-full px-2 py-2 text-left text-sm transition-colors flex items-center gap-2 rounded text-gray-300 hover:bg-gray-800 hover:text-white"
        >
          <action.icon class="w-4 h-4" />
          {action.label}
        </button>
      ))}
    </div>
  );
}

export default class TestPluginClass extends Plugin {
  constructor(engineAPI) {
    super(engineAPI);
  }

  getId() {
    return 'test-plugin';
  }

  getName() {
    return 'Test Plugin';
  }

  getVersion() {
    return '1.0.0';
  }

  getDescription() {
    return 'A simple test plugin demonstrating all UI extension points';
  }

  async onInit() {
    console.log('[TestPlugin] Test plugin initialized');
  }

  async onStart() {
    console.log('[TestPlugin] Registering UI extensions...');
    
    // Register viewport type
    this.registerViewportType('test-viewport', {
      label: 'Test Viewport',
      component: TestViewport,
      icon: IconTestPipe,
      description: 'Test viewport'
    });

    // Register top menu item
    this.registerTopMenuItem('test-menu', {
      label: 'Test',
      icon: IconTestPipe,
      order: 100,
      onClick: () => {
        console.log('[TestPlugin] Test menu clicked');
      }
    });

    // Register property tab
    this.registerPropertyTab('test-properties', {
      title: 'Test Props',
      component: TestPropertiesPanel,
      icon: IconBulb,
      order: 5
    });

    // Register bottom panel tab
    this.registerBottomPanelTab('test-console', {
      title: 'Test Console',
      component: TestConsole,
      icon: IconTerminal,
      order: 15
    });

    // Register regular toolbar button
    this.registerToolbarButton('test-action', {
      title: 'Test Action',
      icon: IconWand,
      section: 'main',
      order: 50,
      onClick: () => {
        console.log('[TestPlugin] Toolbar button clicked!');
        alert('Test plugin toolbar button works!');
      }
    });

    // Register toolbar button with dropdown component
    this.registerToolbarButton('test-dropdown', {
      title: 'Test Actions',
      icon: IconWand,
      section: 'right',
      order: 25, // Position between grid (20) and settings (30)
      hasDropdown: true,
      dropdownComponent: TestDropdownContent,
      dropdownWidth: 192 // w-48 = 192px
    });

    console.log('[TestPlugin] All extensions registered');
  }
}