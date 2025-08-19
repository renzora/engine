# 🎨 Renzora Engine - Theme System

## Custom "renzora" Theme

Your custom DaisyUI theme has been created based on your existing color scheme:

### Color Palette
Based on your original dark theme with slate/gray backgrounds:

- **Primary**: `#0f172a` (Slate-900 - your main background)
- **Secondary**: `#374151` (Gray-700 - your panel backgrounds) 
- **Accent**: `#60a5fa` (Blue-400 - your active/highlight color)
- **Base Colors**: 
  - Base-100: `#111827` (Gray-900 - darkest backgrounds)
  - Base-200: `#1f2937` (Gray-800 - panel backgrounds)
  - Base-300: `#374151` (Gray-700 - lighter panels)
- **Text**: `#e2e8f0` (Slate-200 - your light text)

## How to Use Themes

### 1. Current Theme
The "renzora" theme is set as default in `src/index.html`:
```html
<html lang="en" data-theme="renzora">
```

### 2. Available Themes
- `renzora` (Your custom theme)
- `dark`, `light` (DaisyUI built-ins)
- `cyberpunk`, `synthwave`, `halloween`
- `forest`, `aqua`, `luxury`, `dracula`

### 3. Switch Themes Programmatically
```javascript
// Change theme via JavaScript
document.documentElement.setAttribute('data-theme', 'dark');

// Or use the ThemeSwitcher component
import ThemeSwitcher from '@/ui/ThemeSwitcher.jsx';
```

### 4. Using Theme Colors in Components
```jsx
// Use semantic DaisyUI classes
<div class="bg-primary text-primary-content">Primary Button</div>
<div class="bg-base-200 text-base-content">Panel Background</div>
<div class="text-accent">Accent Text</div>
```

## Customizing the Theme

### 1. Edit Colors
Modify the theme in `tailwind.config.js`:
```javascript
"renzora": {
  "primary": "#your-color-here",
  "secondary": "#your-secondary-color",
  // ... other colors
}
```

### 2. Add New Themes
Add new themes to the `daisyui.themes` array in `tailwind.config.js`.

### 3. Test Theme Changes
After changing colors:
1. Restart your dev server
2. Components will automatically use the new colors
3. All DaisyUI classes will update

## Theme Structure

### Color Categories
1. **Primary/Secondary/Accent**: Main brand colors
2. **Base (100/200/300)**: Background colors (100 = lightest, 300 = darkest)
3. **Neutral**: Text and neutral elements
4. **Semantic**: `info`, `success`, `warning`, `error`
5. **Content**: Text colors that work on their respective backgrounds

### Focus States
Each color has a `-focus` variant for hover/focus states:
- `primary-focus`, `secondary-focus`, etc.

### Content Colors
Each background color has a corresponding `-content` color for text:
- `primary` background → `primary-content` text
- `base-200` background → `base-content` text

Your theme is now fully integrated with DaisyUI and all your layout components!