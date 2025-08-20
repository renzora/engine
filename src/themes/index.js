// Custom Theme Registry
// Import all custom themes to make them available
import './matrix.css';
import './neon.css';
import './ocean.css';
import './terminal.css';
import './aurora.css';
import './volcano.css';

// Theme definitions for the ThemeSwitcher
export const customThemes = [
  { name: 'matrix', label: 'Matrix', category: 'Custom' },
  { name: 'neon', label: 'Neon', category: 'Custom' },
  { name: 'ocean', label: 'Ocean', category: 'Custom' },
  { name: 'terminal', label: 'Terminal', category: 'Custom' },
  { name: 'aurora', label: 'Aurora', category: 'Custom' },
  { name: 'volcano', label: 'Volcano', category: 'Custom' }
];

// All available themes (DaisyUI + Custom)
export const allThemes = [
  // Engine themes
  { name: 'renzora', label: 'Renzora', category: 'Engine' },
  
  // Custom themes
  ...customThemes,
  
  // DaisyUI built-in themes
  { name: 'light', label: 'Light', category: 'DaisyUI' },
  { name: 'dark', label: 'Dark', category: 'DaisyUI' },
  { name: 'cupcake', label: 'Cupcake', category: 'DaisyUI' },
  { name: 'bumblebee', label: 'Bumblebee', category: 'DaisyUI' },
  { name: 'emerald', label: 'Emerald', category: 'DaisyUI' },
  { name: 'corporate', label: 'Corporate', category: 'DaisyUI' },
  { name: 'synthwave', label: 'Synthwave', category: 'DaisyUI' },
  { name: 'retro', label: 'Retro', category: 'DaisyUI' },
  { name: 'cyberpunk', label: 'Cyberpunk', category: 'DaisyUI' },
  { name: 'valentine', label: 'Valentine', category: 'DaisyUI' },
  { name: 'halloween', label: 'Halloween', category: 'DaisyUI' },
  { name: 'garden', label: 'Garden', category: 'DaisyUI' },
  { name: 'forest', label: 'Forest', category: 'DaisyUI' },
  { name: 'aqua', label: 'Aqua', category: 'DaisyUI' },
  { name: 'lofi', label: 'Lo-Fi', category: 'DaisyUI' },
  { name: 'pastel', label: 'Pastel', category: 'DaisyUI' },
  { name: 'fantasy', label: 'Fantasy', category: 'DaisyUI' },
  { name: 'wireframe', label: 'Wireframe', category: 'DaisyUI' },
  { name: 'black', label: 'Black', category: 'DaisyUI' },
  { name: 'luxury', label: 'Luxury', category: 'DaisyUI' },
  { name: 'dracula', label: 'Dracula', category: 'DaisyUI' },
  { name: 'cmyk', label: 'CMYK', category: 'DaisyUI' },
  { name: 'autumn', label: 'Autumn', category: 'DaisyUI' },
  { name: 'business', label: 'Business', category: 'DaisyUI' },
  { name: 'acid', label: 'Acid', category: 'DaisyUI' },
  { name: 'lemonade', label: 'Lemonade', category: 'DaisyUI' },
  { name: 'night', label: 'Night', category: 'DaisyUI' },
  { name: 'coffee', label: 'Coffee', category: 'DaisyUI' },
  { name: 'winter', label: 'Winter', category: 'DaisyUI' },
  { name: 'dim', label: 'Dim', category: 'DaisyUI' },
  { name: 'nord', label: 'Nord', category: 'DaisyUI' },
  { name: 'sunset', label: 'Sunset', category: 'DaisyUI' }
];

export default allThemes;