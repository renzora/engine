# RenScript File Icon Setup

## Method 1: Built-in Icon (Already Configured)
The extension now includes a built-in icon for .ren files. After reloading VS Code, you should see the custom icon in:
- File explorer
- Editor tabs
- Quick open dialog

## Method 2: Adding to Popular Icon Themes

If you're using a custom icon theme and want to add RenScript icons to it, here's how:

### For Material Icon Theme:
1. Open VS Code settings (Ctrl+,)
2. Search for "Material Icon Theme: Files Associations"
3. Click "Edit in settings.json"
4. Add this to the configuration:

```json
"material-icon-theme.files.associations": {
  "*.ren": "script"
}
```

### For VSCode Icons:
1. Open settings.json
2. Add:

```json
"vsicons.associations.files": [
  {
    "icon": "script",
    "extensions": ["ren"],
    "format": "svg"
  }
]
```

### For Bearded Icons:
1. Open settings.json
2. Add:

```json
"beardedIcons.fileAssociations": {
  "*.ren": "script"
}
```

## Method 3: Custom File Icon Theme

If you want to use the custom RenScript icon with any theme, you can:

1. Install the "File Icon Theme" extension
2. Configure it to use the RenScript icon:

```json
"fileIcons.associations": {
  "*.ren": "../extensions/vscode-renscript/images/renscript-icon.svg"
}
```

## Reload VS Code

After making any changes:
1. Press `Ctrl+Shift+P`
2. Type "Developer: Reload Window"
3. Press Enter

Your .ren files should now display with the custom icon!