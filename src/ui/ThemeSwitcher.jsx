import { createSignal, onMount } from 'solid-js';

const ThemeSwitcher = () => {
  const [currentTheme, setCurrentTheme] = createSignal('renzora');
  
  const themes = [
    { name: 'renzora', label: 'Renzora' },
    { name: 'light', label: 'Light' },
    { name: 'dark', label: 'Dark' },
    { name: 'cupcake', label: 'Cupcake' },
    { name: 'bumblebee', label: 'Bumblebee' },
    { name: 'emerald', label: 'Emerald' },
    { name: 'corporate', label: 'Corporate' },
    { name: 'synthwave', label: 'Synthwave' },
    { name: 'retro', label: 'Retro' },
    { name: 'cyberpunk', label: 'Cyberpunk' },
    { name: 'valentine', label: 'Valentine' },
    { name: 'halloween', label: 'Halloween' },
    { name: 'garden', label: 'Garden' },
    { name: 'forest', label: 'Forest' },
    { name: 'aqua', label: 'Aqua' },
    { name: 'lofi', label: 'Lo-Fi' },
    { name: 'pastel', label: 'Pastel' },
    { name: 'fantasy', label: 'Fantasy' },
    { name: 'wireframe', label: 'Wireframe' },
    { name: 'black', label: 'Black' },
    { name: 'luxury', label: 'Luxury' },
    { name: 'dracula', label: 'Dracula' },
    { name: 'cmyk', label: 'CMYK' },
    { name: 'autumn', label: 'Autumn' },
    { name: 'business', label: 'Business' },
    { name: 'acid', label: 'Acid' },
    { name: 'lemonade', label: 'Lemonade' },
    { name: 'night', label: 'Night' },
    { name: 'coffee', label: 'Coffee' },
    { name: 'winter', label: 'Winter' },
    { name: 'dim', label: 'Dim' },
    { name: 'nord', label: 'Nord' },
    { name: 'sunset', label: 'Sunset' }
  ];

  onMount(() => {
    const html = document.documentElement;
    const theme = html.getAttribute('data-theme') || 'renzora';
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
        <span>🎨 {themes.find(t => t.name === currentTheme())?.label || 'Theme'}</span>
        <svg class={`w-3 h-3 transition-transform ${isOpen() ? 'rotate-180' : ''}`} fill="currentColor" viewBox="0 0 20 20">
          <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
        </svg>
      </button>
      
      {isOpen() && (
        <div class="absolute top-full right-0 mt-1 w-32 bg-base-200 border border-base-300 rounded shadow-xl z-50 max-h-48 overflow-y-auto">
          {themes.map(theme => (
            <button
              onClick={() => {
                handleThemeChange(theme.name);
                setIsOpen(false);
              }}
              class={`w-full px-2 py-1 text-left text-xs transition-colors hover:bg-base-300 first:rounded-t last:rounded-b ${
                currentTheme() === theme.name ? 'bg-primary text-primary-content' : 'text-base-content'
              }`}
            >
              <span class="flex items-center justify-between">
                <span class="truncate">{theme.label}</span>
                {currentTheme() === theme.name && <span class="text-xs">✓</span>}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
};

export default ThemeSwitcher;