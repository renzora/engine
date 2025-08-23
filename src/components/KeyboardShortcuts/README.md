# Keyboard Shortcuts System

A centralized keyboard shortcut management system that can be disabled when text inputs (like Monaco Editor) are focused.

## Usage

### 1. Add the KeyboardShortcuts component to your app root

```jsx
import KeyboardShortcuts from '@/components/KeyboardShortcuts';

function App() {
  return (
    <div>
      <KeyboardShortcuts />
      {/* Your app content */}
    </div>
  );
}
```

### 2. Register shortcuts using the hook

```jsx
import { useGameEngineShortcuts } from '@/hooks/useGameEngineShortcuts';

function EditorPage() {
  useGameEngineShortcuts({
    moveForward: () => console.log('Moving forward'),
    moveLeft: () => console.log('Moving left'),
    save: () => console.log('Saving'),
    // ... other callbacks
  });

  return <div>Your editor content</div>;
}
```

### 3. Manual shortcut registration

```jsx
import { keyboardShortcuts } from '@/components/KeyboardShortcuts';

onMount(() => {
  const customHandler = keyboardShortcuts.createHandler({
    'ctrl+k': () => console.log('Custom shortcut'),
    'f1': () => showHelp(),
  });
  
  const unregister = keyboardShortcuts.register(customHandler);
  
  onCleanup(unregister);
});
```

## API

- `keyboardShortcuts.disable()` - Disable all shortcuts
- `keyboardShortcuts.enable()` - Enable all shortcuts  
- `keyboardShortcuts.register(handler)` - Register a handler function
- `keyboardShortcuts.createHandler(shortcuts)` - Create handler from shortcut object
- `keyboardShortcuts.isDisabled()` - Check if shortcuts are disabled

## Key Pattern Format

- Single keys: `'w'`, `'space'`, `'f1'`
- With modifiers: `'ctrl+s'`, `'shift+w'`, `'alt+f4'`
- Multiple modifiers: `'ctrl+shift+z'`

## Integration with Monaco Editor

The Monaco Editor automatically disables shortcuts when focused and re-enables them when blurred.