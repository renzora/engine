# Custom Themes

This directory contains custom themes for the Renzora Engine, built on top of DaisyUI's theming system.

## Available Custom Themes

### 🔰 Matrix Theme (`matrix`)
- **Style**: Inspired by The Matrix movie
- **Colors**: Bright green on dark background
- **Features**: 
  - Monospace font (Courier New)
  - Green glow effects
  - Terminal-like appearance
  - Subtle text shadows

### 🌈 Neon Theme (`neon`) 
- **Style**: Cyberpunk with vibrant neon colors
- **Colors**: Magenta, cyan, and yellow neon colors
- **Features**:
  - Bright glowing effects
  - Neon text shadows
  - High contrast elements
  - Futuristic feel

### 🌊 Ocean Theme (`ocean`)
- **Style**: Deep sea inspired
- **Colors**: Various shades of blue and teal
- **Features**:
  - Wave-like box shadows
  - Gradient backgrounds
  - Shimmer effects on hover
  - Smooth transitions

### 💻 Terminal Theme (`terminal`)
- **Style**: Classic terminal/console
- **Colors**: Bright green text on black background  
- **Features**:
  - Pure monospace typography
  - Scan line overlay effect
  - No rounded corners (rectangular design)
  - Terminal glow effects

### 🌟 Aurora Theme (`aurora`)
- **Style**: Northern lights inspired
- **Colors**: Purple, blue, and green gradients
- **Features**:
  - Animated background gradients
  - Shimmer and glow effects
  - Backdrop blur for glass-morphism
  - Smooth color transitions

### 🌋 Volcano Theme (`volcano`)
- **Style**: Fiery lava inspired
- **Colors**: Red, orange, and yellow fire colors
- **Features**:
  - Lava-like gradient backgrounds
  - Heat shimmer effects
  - Animated glowing elements
  - Fire color palette

## Usage

Themes are automatically imported and available in the ThemeSwitcher component. They are organized by category:

- **Engine**: Default Renzora theme
- **Custom**: All custom themes listed above  
- **DaisyUI**: Built-in DaisyUI themes

## Creating New Themes

To create a new custom theme:

1. Create a new CSS file in this directory (e.g., `mytheme.css`)
2. Define your theme using DaisyUI 5's CSS variable system with OKLCH colors:

```css
[data-theme="mytheme"] {
  color-scheme: dark; /* or light */
  
  /* Base colors */
  --color-base-100: oklch(5% 0.05 240);      /* background */
  --color-base-200: oklch(10% 0.05 240);     /* slightly lighter */
  --color-base-300: oklch(15% 0.05 240);     /* borders */
  --color-base-content: oklch(90% 0.10 240); /* text */
  
  /* Primary colors */
  --color-primary: oklch(70% 0.25 200);      /* main brand color */
  --color-primary-content: oklch(15% 0.05 200); /* text on primary */
  
  /* Secondary colors */
  --color-secondary: oklch(65% 0.20 160);    /* secondary brand color */
  --color-secondary-content: oklch(15% 0.05 160); /* text on secondary */
  
  /* Accent colors */
  --color-accent: oklch(75% 0.25 300);       /* accent color */
  --color-accent-content: oklch(15% 0.05 300); /* text on accent */
  
  /* Neutral colors */
  --color-neutral: oklch(25% 0.02 240);      /* neutral elements */
  --color-neutral-content: oklch(85% 0.10 240); /* text on neutral */
  
  /* Functional colors */
  --color-info: oklch(70% 0.25 220);         /* info messages */
  --color-info-content: oklch(15% 0.05 220);
  --color-success: oklch(65% 0.20 130);      /* success messages */
  --color-success-content: oklch(15% 0.05 130);
  --color-warning: oklch(75% 0.25 60);       /* warning messages */
  --color-warning-content: oklch(15% 0.05 60);
  --color-error: oklch(65% 0.25 20);         /* error messages */
  --color-error-content: oklch(15% 0.05 20);
  
  /* Shape and sizing */
  --radius-box: 0.5rem;      /* border radius for cards */
  --radius-field: 0.25rem;   /* border radius for inputs */
  --radius-selector: 0.25rem; /* border radius for checkboxes */
  --size-field: 2.5rem;      /* height of inputs */
  --size-selector: 1.25rem;  /* size of checkboxes */
  --border: 1px;             /* border width */
}

/* Custom styling */
[data-theme="mytheme"] .custom-element {
  /* Your custom styles */
}
```

**Important Notes:**
- Use OKLCH color format: `oklch(lightness% chroma hue)`
- Lightness: 0-100% (0% = black, 100% = white)
- Chroma: 0-0.4+ (0 = gray, higher = more saturated)
- Hue: 0-360 degrees (color wheel position)
- Content colors should contrast well with their base colors

3. Add the theme to `src/themes/index.js`:

```javascript
import './mytheme.css';

export const customThemes = [
  // ... existing themes
  { name: 'mytheme', label: 'My Theme', category: 'Custom' }
];
```

## DaisyUI Integration

These themes extend DaisyUI's theming system by:

- Using DaisyUI's CSS variable structure
- Adding custom effects and animations
- Maintaining compatibility with DaisyUI components
- Following responsive design principles

For more information on DaisyUI theming, see: https://daisyui.com/docs/themes/