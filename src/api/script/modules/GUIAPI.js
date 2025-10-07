// === GUI API MODULE ===

import {
  AdvancedDynamicTexture,
  Rectangle,
  Button,
  TextBlock,
  Image,
  StackPanel,
  Grid,
  ScrollViewer,
  Ellipse,
  Line,
  Slider,
  Checkbox,
  RadioButton,
  InputText,
  Control,
  Container,
  ColorPicker
} from '@babylonjs/gui';

import { Color3 } from '@babylonjs/core';

export class GUIAPI {
  constructor(scene) {
    this.scene = scene;
    this.fullscreenUI = null;
  }

  // === GUI SETUP ===

  createFullscreenUI(name = 'UI') {
    this.fullscreenUI = AdvancedDynamicTexture.CreateFullscreenUI(name);
    return this.fullscreenUI;
  }

  createTextureUI(name, width = 1024, height = 1024) {
    return AdvancedDynamicTexture.CreateForMesh(null, width, height);
  }

  createMeshUI(mesh, width = 1024, height = 1024) {
    if (!mesh) return null;
    return AdvancedDynamicTexture.CreateForMesh(mesh, width, height);
  }

  getUI() {
    return this.fullscreenUI || this.createFullscreenUI();
  }

  // === BASIC CONTROLS ===

  createButton(name, text = '', width = '150px', height = '40px') {
    const button = Button.CreateSimpleButton(name, text);
    button.widthInPixels = parseInt(width);
    button.heightInPixels = parseInt(height);
    button.color = 'white';
    button.cornerRadius = 5;
    button.background = 'blue';
    return button;
  }

  createImageButton(name, imageUrl, width = '150px', height = '40px') {
    const button = Button.CreateImageButton(name, '', imageUrl);
    button.widthInPixels = parseInt(width);
    button.heightInPixels = parseInt(height);
    return button;
  }

  createTextBlock(name, text = '', fontSize = 24, color = 'white') {
    const textBlock = new TextBlock(name, text);
    textBlock.fontSize = fontSize;
    textBlock.color = color;
    return textBlock;
  }

  createImage(name, url, width = '100px', height = '100px') {
    const image = new Image(name, url);
    image.widthInPixels = parseInt(width);
    image.heightInPixels = parseInt(height);
    return image;
  }

  createRectangle(name, width = '100px', height = '100px', color = 'white') {
    const rect = new Rectangle(name);
    rect.widthInPixels = parseInt(width);
    rect.heightInPixels = parseInt(height);
    rect.color = color;
    rect.thickness = 2;
    return rect;
  }

  createEllipse(name, width = '100px', height = '100px', color = 'white') {
    const ellipse = new Ellipse(name);
    ellipse.widthInPixels = parseInt(width);
    ellipse.heightInPixels = parseInt(height);
    ellipse.color = color;
    ellipse.thickness = 2;
    return ellipse;
  }

  createLine(name, x1 = 0, y1 = 0, x2 = 100, y2 = 100, color = 'white') {
    const line = new Line(name);
    line.x1 = x1;
    line.y1 = y1;
    line.x2 = x2;
    line.y2 = y2;
    line.color = color;
    line.lineWidth = 2;
    return line;
  }

  // === INPUT CONTROLS ===

  createSlider(name, min = 0, max = 100, value = 50, width = '200px', height = '20px') {
    const slider = new Slider(name);
    slider.minimum = min;
    slider.maximum = max;
    slider.value = value;
    slider.widthInPixels = parseInt(width);
    slider.heightInPixels = parseInt(height);
    slider.color = 'blue';
    slider.background = 'gray';
    return slider;
  }

  createCheckbox(name, text = '', isChecked = false) {
    const checkbox = new Checkbox(name);
    checkbox.width = '20px';
    checkbox.height = '20px';
    checkbox.isChecked = isChecked;
    checkbox.color = 'green';
    checkbox.background = 'white';
    
    if (text) {
      const label = this.createTextBlock(`${name}_label`, text, 16, 'white');
      label.paddingLeft = '30px';
      
      const container = new StackPanel(`${name}_container`);
      container.isVertical = false;
      container.addControl(checkbox);
      container.addControl(label);
      
      return { checkbox, label, container };
    }
    
    return checkbox;
  }

  createRadioButton(name, group, text = '', isChecked = false) {
    const radioButton = new RadioButton(name);
    radioButton.width = '20px';
    radioButton.height = '20px';
    radioButton.group = group;
    radioButton.isChecked = isChecked;
    radioButton.color = 'green';
    radioButton.background = 'white';
    
    if (text) {
      const label = this.createTextBlock(`${name}_label`, text, 16, 'white');
      label.paddingLeft = '30px';
      
      const container = new StackPanel(`${name}_container`);
      container.isVertical = false;
      container.addControl(radioButton);
      container.addControl(label);
      
      return { radioButton, label, container };
    }
    
    return radioButton;
  }

  createInputText(name, placeholder = '', width = '200px', height = '30px') {
    const input = new InputText(name, placeholder);
    input.widthInPixels = parseInt(width);
    input.heightInPixels = parseInt(height);
    input.color = 'white';
    input.background = 'black';
    input.focusedBackground = 'darkblue';
    return input;
  }

  createColorPicker(name, value = '#ffffff', width = '150px', height = '150px') {
    const colorPicker = new ColorPicker(name);
    colorPicker.value = Color3.FromHexString(value);
    colorPicker.widthInPixels = parseInt(width);
    colorPicker.heightInPixels = parseInt(height);
    return colorPicker;
  }

  // === LAYOUT CONTROLS ===

  createStackPanel(name, isVertical = true) {
    const panel = new StackPanel(name);
    panel.isVertical = isVertical;
    return panel;
  }

  createGrid(name, rows = 2, columns = 2) {
    const grid = new Grid(name);
    
    // Add rows
    for (let i = 0; i < rows; i++) {
      grid.addRowDefinition(1 / rows);
    }
    
    // Add columns  
    for (let i = 0; i < columns; i++) {
      grid.addColumnDefinition(1 / columns);
    }
    
    return grid;
  }

  createScrollViewer(name, width = '300px', height = '200px') {
    const scrollViewer = new ScrollViewer(name);
    scrollViewer.widthInPixels = parseInt(width);
    scrollViewer.heightInPixels = parseInt(height);
    scrollViewer.color = 'white';
    scrollViewer.background = 'black';
    return scrollViewer;
  }

  createContainer(name, width = '200px', height = '200px') {
    const container = new Container(name);
    container.widthInPixels = parseInt(width);
    container.heightInPixels = parseInt(height);
    return container;
  }

  // === GUI POSITIONING ===

  setControlPosition(control, x, y, unit = 'px') {
    if (!control) return false;
    
    if (unit === 'px') {
      control.leftInPixels = x;
      control.topInPixels = y;
    } else {
      control.left = x + unit;
      control.top = y + unit;
    }
    return true;
  }

  setControlSize(control, width, height, unit = 'px') {
    if (!control) return false;
    
    if (unit === 'px') {
      control.widthInPixels = width;
      control.heightInPixels = height;
    } else {
      control.width = width + unit;
      control.height = height + unit;
    }
    return true;
  }

  setControlAlignment(control, horizontalAlignment = 2, verticalAlignment = 2) {
    if (!control) return false;
    // Control.HORIZONTAL_ALIGNMENT_LEFT = 0, CENTER = 1, RIGHT = 2
    // Control.VERTICAL_ALIGNMENT_TOP = 0, CENTER = 1, BOTTOM = 2
    control.horizontalAlignment = horizontalAlignment;
    control.verticalAlignment = verticalAlignment;
    return true;
  }

  setControlPadding(control, top = 0, right = 0, bottom = 0, left = 0) {
    if (!control) return false;
    control.paddingTop = top + 'px';
    control.paddingRight = right + 'px';
    control.paddingBottom = bottom + 'px';
    control.paddingLeft = left + 'px';
    return true;
  }

  // === GUI EVENTS ===

  onButtonClick(button, callback) {
    if (!button || !callback) return false;
    button.onPointerUpObservable.add(callback);
    return true;
  }

  onButtonHover(button, hoverCallback, outCallback = null) {
    if (!button || !hoverCallback) return false;
    
    button.onPointerEnterObservable.add(hoverCallback);
    if (outCallback) {
      button.onPointerOutObservable.add(outCallback);
    }
    return true;
  }

  onSliderChange(slider, callback) {
    if (!slider || !callback) return false;
    slider.onValueChangedObservable.add(callback);
    return true;
  }

  onTextInput(inputText, callback) {
    if (!inputText || !callback) return false;
    inputText.onTextChangedObservable.add(callback);
    return true;
  }

  onCheckboxChange(checkbox, callback) {
    if (!checkbox || !callback) return false;
    checkbox.onIsCheckedChangedObservable.add(callback);
    return true;
  }

  onColorPickerChange(colorPicker, callback) {
    if (!colorPicker || !callback) return false;
    colorPicker.onValueChangedObservable.add(callback);
    return true;
  }

  // === GUI STYLING ===

  setControlBackground(control, background) {
    if (!control) return false;
    control.background = background;
    return true;
  }

  setControlBorder(control, color = 'white', thickness = 2) {
    if (!control) return false;
    control.color = color;
    control.thickness = thickness;
    return true;
  }

  setControlCornerRadius(control, radius = 0) {
    if (!control) return false;
    control.cornerRadius = radius;
    return true;
  }

  setControlShadow(control, blur = 5, color = 'black', offsetX = 2, offsetY = 2) {
    if (!control) return false;
    control.shadowBlur = blur;
    control.shadowColor = color;
    control.shadowOffsetX = offsetX;
    control.shadowOffsetY = offsetY;
    return true;
  }

  setControlAlpha(control, alpha = 1.0) {
    if (!control) return false;
    control.alpha = Math.max(0, Math.min(1, alpha));
    return true;
  }

  setControlVisible(control, visible = true) {
    if (!control) return false;
    control.isVisible = visible;
    return true;
  }

  // === GUI HIERARCHY ===

  addControlToContainer(container, control) {
    if (!container || !control) return false;
    container.addControl(control);
    return true;
  }

  addControlToGrid(grid, control, row = 0, column = 0) {
    if (!grid || !control) return false;
    grid.addControl(control, row, column);
    return true;
  }

  removeControlFromContainer(container, control) {
    if (!container || !control) return false;
    container.removeControl(control);
    return true;
  }

  addControlToUI(control, ui = null) {
    if (!control) return false;
    const targetUI = ui || this.getUI();
    targetUI.addControl(control);
    return true;
  }

  // === GUI ANIMATIONS ===

  animateControlProperty(control, property, targetValue, duration = 1000, easingFunction = null) {
    if (!control) return false;
    
    const startValue = control[property];
    const startTime = Date.now();
    
    const animate = () => {
      const elapsed = Date.now() - startTime;
      const progress = Math.min(elapsed / duration, 1);
      
      let easedProgress = progress;
      if (easingFunction) {
        easedProgress = easingFunction(progress);
      }
      
      const currentValue = startValue + (targetValue - startValue) * easedProgress;
      control[property] = currentValue;
      
      if (progress < 1) {
        requestAnimationFrame(animate);
      }
    };
    
    requestAnimationFrame(animate);
    return true;
  }

  fadeInControl(control, duration = 500) {
    if (!control) return false;
    control.alpha = 0;
    control.isVisible = true;
    return this.animateControlProperty(control, 'alpha', 1, duration);
  }

  fadeOutControl(control, duration = 500) {
    if (!control) return false;
    
    this.animateControlProperty(control, 'alpha', 0, duration);
    setTimeout(() => {
      control.isVisible = false;
    }, duration);
    
    return true;
  }

  slideInControl(control, direction = 'left', duration = 500) {
    if (!control) return false;
    
    const originalLeft = control.leftInPixels;
    const originalTop = control.topInPixels;
    
    switch (direction) {
      case 'left':
        control.leftInPixels = -control.widthInPixels;
        break;
      case 'right':
        control.leftInPixels = window.innerWidth;
        break;
      case 'top':
        control.topInPixels = -control.heightInPixels;
        break;
      case 'bottom':
        control.topInPixels = window.innerHeight;
        break;
    }
    
    control.isVisible = true;
    
    if (direction === 'left' || direction === 'right') {
      return this.animateControlProperty(control, 'leftInPixels', originalLeft, duration);
    } else {
      return this.animateControlProperty(control, 'topInPixels', originalTop, duration);
    }
  }

  // === GUI DIALOGS ===

  createDialog(title = 'Dialog', message = 'Message', buttons = ['OK']) {
    const dialog = new Rectangle('dialog');
    dialog.widthInPixels = 400;
    dialog.heightInPixels = 200;
    dialog.background = 'rgba(0, 0, 0, 0.8)';
    dialog.color = 'white';
    dialog.thickness = 2;
    dialog.cornerRadius = 10;
    
    const titleText = this.createTextBlock('dialog_title', title, 20, 'white');
    titleText.top = '-60px';
    
    const messageText = this.createTextBlock('dialog_message', message, 16, 'white');
    messageText.top = '-20px';
    messageText.textWrapping = true;
    
    const buttonContainer = this.createStackPanel('dialog_buttons', false);
    buttonContainer.top = '40px';
    buttonContainer.spacing = '10px';
    
    const buttonElements = [];
    buttons.forEach((buttonText, index) => {
      const button = this.createButton(`dialog_button_${index}`, buttonText, '80px', '30px');
      buttonContainer.addControl(button);
      buttonElements.push(button);
    });
    
    dialog.addControl(titleText);
    dialog.addControl(messageText);
    dialog.addControl(buttonContainer);
    
    return {
      dialog,
      title: titleText,
      message: messageText,
      buttons: buttonElements,
      show: () => {
        this.getUI().addControl(dialog);
        return true;
      },
      hide: () => {
        this.getUI().removeControl(dialog);
        return true;
      }
    };
  }

  createProgressBar(name, width = '300px', height = '20px', value = 0) {
    const background = new Rectangle(`${name}_bg`);
    background.widthInPixels = parseInt(width);
    background.heightInPixels = parseInt(height);
    background.background = 'gray';
    background.color = 'white';
    background.thickness = 1;
    
    const fill = new Rectangle(`${name}_fill`);
    fill.widthInPixels = parseInt(width) * Math.max(0, Math.min(1, value));
    fill.heightInPixels = parseInt(height) - 2;
    fill.background = 'blue';
    fill.horizontalAlignment = Control.HORIZONTAL_ALIGNMENT_LEFT;
    
    const text = this.createTextBlock(`${name}_text`, `${Math.round(value * 100)}%`, 14, 'white');
    
    background.addControl(fill);
    background.addControl(text);
    
    return {
      container: background,
      fill,
      text,
      setValue: (newValue) => {
        const clampedValue = Math.max(0, Math.min(1, newValue));
        fill.widthInPixels = parseInt(width) * clampedValue;
        text.text = `${Math.round(clampedValue * 100)}%`;
      }
    };
  }

  // === GUI MENUS ===

  createContextMenu(name, items = []) {
    const menu = new Rectangle(`${name}_menu`);
    menu.background = 'rgba(0, 0, 0, 0.9)';
    menu.color = 'white';
    menu.thickness = 1;
    menu.cornerRadius = 5;
    menu.isVisible = false;
    
    const itemContainer = this.createStackPanel(`${name}_items`, true);
    menu.addControl(itemContainer);
    
    const menuItems = [];
    items.forEach((item, index) => {
      const button = this.createButton(`${name}_item_${index}`, item.text, '100%', '30px');
      button.background = 'transparent';
      button.color = 'white';
      
      if (item.callback) {
        this.onButtonClick(button, () => {
          item.callback();
          menu.isVisible = false;
        });
      }
      
      itemContainer.addControl(button);
      menuItems.push(button);
    });
    
    // Auto-size menu
    menu.heightInPixels = items.length * 30 + 10;
    menu.widthInPixels = 150;
    
    return {
      menu,
      items: menuItems,
      show: (x, y) => {
        menu.leftInPixels = x;
        menu.topInPixels = y;
        menu.isVisible = true;
        this.getUI().addControl(menu);
      },
      hide: () => {
        menu.isVisible = false;
      }
    };
  }

  // === GUI UTILITIES ===

  setControlDraggable(control, enabled = true) {
    if (!control) return false;
    
    if (enabled) {
      let isDragging = false;
      let startPointer = { x: 0, y: 0 };
      let startPosition = { x: 0, y: 0 };
      
      control.onPointerDownObservable.add((eventData) => {
        isDragging = true;
        startPointer = { x: eventData.x, y: eventData.y };
        startPosition = { x: control.leftInPixels, y: control.topInPixels };
      });
      
      control.onPointerMoveObservable.add((eventData) => {
        if (isDragging) {
          const deltaX = eventData.x - startPointer.x;
          const deltaY = eventData.y - startPointer.y;
          control.leftInPixels = startPosition.x + deltaX;
          control.topInPixels = startPosition.y + deltaY;
        }
      });
      
      control.onPointerUpObservable.add(() => {
        isDragging = false;
      });
    }
    
    return true;
  }

  bringControlToFront(control) {
    if (!control) return false;
    control.zIndex = 1000;
    return true;
  }

  sendControlToBack(control) {
    if (!control) return false;
    control.zIndex = -1000;
    return true;
  }

  // === GUI INFO ===

  getControlInfo(control) {
    if (!control) return null;
    
    return {
      name: control.name,
      type: control.getClassName(),
      visible: control.isVisible,
      enabled: control.isEnabled,
      position: {
        left: control.leftInPixels,
        top: control.topInPixels
      },
      size: {
        width: control.widthInPixels,
        height: control.heightInPixels
      },
      alpha: control.alpha,
      zIndex: control.zIndex
    };
  }

  getAllControls(container = null) {
    const targetContainer = container || this.getUI();
    const controls = [];
    
    const collectControls = (control) => {
      controls.push({
        name: control.name,
        type: control.getClassName(),
        visible: control.isVisible
      });
      
      if (control.children) {
        control.children.forEach(child => collectControls(child));
      }
    };
    
    if (targetContainer.children) {
      targetContainer.children.forEach(child => collectControls(child));
    }
    
    return controls;
  }

  findControlByName(name, container = null) {
    const targetContainer = container || this.getUI();
    return targetContainer.getControlByName(name);
  }

  // === GUI THEMES ===

  applyDarkTheme(controls = []) {
    const theme = {
      background: '#2d2d2d',
      foreground: '#ffffff',
      accent: '#007acc',
      border: '#555555'
    };
    
    const targetControls = controls.length > 0 ? controls : this.getAllControlsFlat();
    
    targetControls.forEach(control => {
      if (control.background !== undefined) {
        control.background = theme.background;
      }
      if (control.color !== undefined) {
        control.color = theme.foreground;
      }
    });
    
    return true;
  }

  applyLightTheme(controls = []) {
    const theme = {
      background: '#f0f0f0',
      foreground: '#000000',
      accent: '#0078d4',
      border: '#cccccc'
    };
    
    const targetControls = controls.length > 0 ? controls : this.getAllControlsFlat();
    
    targetControls.forEach(control => {
      if (control.background !== undefined) {
        control.background = theme.background;
      }
      if (control.color !== undefined) {
        control.color = theme.foreground;
      }
    });
    
    return true;
  }

  getAllControlsFlat(container = null) {
    const targetContainer = container || this.getUI();
    const controls = [];
    
    const collectControls = (control) => {
      controls.push(control);
      if (control.children) {
        control.children.forEach(child => collectControls(child));
      }
    };
    
    if (targetContainer.children) {
      targetContainer.children.forEach(child => collectControls(child));
    }
    
    return controls;
  }

  // === GUI DISPOSAL ===

  disposeControl(control) {
    if (!control) return false;
    control.dispose();
    return true;
  }

  clearAllControls(container = null) {
    const targetContainer = container || this.getUI();
    const controls = [...(targetContainer.children || [])];
    controls.forEach(control => control.dispose());
    return true;
  }
}