import { createSignal, onMount, For } from 'solid-js';
import { allThemes } from '../themes/index.js';

const ThemeSwitcher = () => {
  const [currentTheme, setCurrentTheme] = createSignal('dark');
  
  // Group themes by category
  const themesByCategory = () => {
    const grouped = {};
    allThemes.forEach(theme => {
      if (!grouped[theme.category]) {
        grouped[theme.category] = [];
      }
      grouped[theme.category].push(theme);
    });
    return grouped;
  };

  onMount(() => {
    const html = document.documentElement;
    const savedTheme = localStorage.getItem('theme');
    const theme = savedTheme || html.getAttribute('data-theme') || 'dark';
    
    // Apply the theme to the DOM
    html.setAttribute('data-theme', theme);
    setCurrentTheme(theme);
  });

  const handleThemeChange = (themeName) => {
    const html = document.documentElement;
    html.setAttribute('data-theme', themeName);
    setCurrentTheme(themeName);
    
    // Save to localStorage for persistence
    localStorage.setItem('theme', themeName);
  };

  const [isOpen, setIsOpen] = createSignal(false);

  return (
    <div class="relative">
      <button
        onClick={() => setIsOpen(!isOpen())}
        class="px-2 py-1 text-xs bg-base-200 text-base-content rounded border border-base-300 hover:bg-base-300 transition-colors flex items-center gap-1"
      >
        <span>🎨 {allThemes.find(t => t.name === currentTheme())?.label || 'Theme'}</span>
        <svg class={`w-3 h-3 transition-transform ${isOpen() ? 'rotate-180' : ''}`} fill="currentColor" viewBox="0 0 20 20">
          <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
        </svg>
      </button>
      
      {isOpen() && (
        <div class="absolute top-full right-0 mt-1 w-44 bg-base-200 border border-base-300 rounded shadow-xl z-50 max-h-64 overflow-y-auto">
          <For each={Object.entries(themesByCategory())}>
            {([category, themes]) => (
              <div class="border-b border-base-300 last:border-b-0">
                <div class="px-2 py-1 text-[10px] font-semibold text-base-content/60 uppercase tracking-wide bg-base-100">
                  {category}
                </div>
                <For each={themes}>
                  {(theme) => (
                    <button
                      onClick={() => {
                        handleThemeChange(theme.name);
                        setIsOpen(false);
                      }}
                      class={`w-full px-2 py-1 text-left text-xs transition-colors hover:bg-base-300 ${
                        currentTheme() === theme.name ? 'bg-primary text-primary-content' : 'text-base-content'
                      }`}
                    >
                      <span class="flex items-center justify-between">
                        <span class="truncate">{theme.label}</span>
                        {currentTheme() === theme.name && <span class="text-xs">✓</span>}
                      </span>
                    </button>
                  )}
                </For>
              </div>
            )}
          </For>
        </div>
      )}
    </div>
  );
};

export default ThemeSwitcher;