input = {
  eventListeners: {},

  assign: function(type, callback, options = {}) {
    if (type.includes('+')) {
      const parts = type.split('+');
      const mainType = parts[0];
      const modifiers = parts.slice(1, -1);
      const mainKey = parts[parts.length - 1].toLowerCase();

      const comboListener = (e) => {
        for (let mod of modifiers) {
          mod = mod.toLowerCase();
          if (mod === 'shift' && !e.shiftKey) return;
          if (mod === 'ctrl'  && !e.ctrlKey)  return;
          if (mod === 'alt'   && !e.altKey)   return;
          if (mod === 'meta'  && !e.metaKey)  return;
        }
        if (e.key.toLowerCase() === mainKey) {
          console.log(`input.assign -> Handling ${type}`);
          callback(e);
        }
      };

      if (!this.eventListeners[mainType]) {
        this.eventListeners[mainType] = new Set();
      }
      if (!this.eventListeners[mainType].has(comboListener)) {
        this.eventListeners[mainType].add(comboListener);
        window.addEventListener(mainType, comboListener, options);
        console.log(`input.assign -> Added combo listener for "${type}"`);
      }

    } else {
      let [mainType, subKey] = type.split(".");

      if (subKey) {
        const keyName = subKey.toLowerCase();
        const keyListener = (e) => {
          if (e.key.toLowerCase() === keyName) {
            if (["tab", " "].includes(keyName)) {
              e.preventDefault();
            }
            console.log(`input.assign -> Handling ${mainType}.${keyName}`);
            callback(e);
          }
        };

        if (!this.eventListeners[mainType]) {
          this.eventListeners[mainType] = new Set();
        }
        if (!this.eventListeners[mainType].has(keyListener)) {
          this.eventListeners[mainType].add(keyListener);
          window.addEventListener(mainType, keyListener, options);
          console.log(`input.assign -> Added event listener for ${mainType}.${keyName}`);
        }
      } else {
        if (!this.eventListeners[type]) {
          this.eventListeners[type] = new Set();
        }
        if (!this.eventListeners[type].has(callback)) {
          this.eventListeners[type].add(callback);
          window.addEventListener(type, callback, options);
          console.log(`input.assign -> Added event listener for ${type}`);
        }
      }
    }
  },

  unassign: function(type, callback) {
    if (this.eventListeners[type] && this.eventListeners[type].has(callback)) {
      this.eventListeners[type].delete(callback);
      window.removeEventListener(type, callback);
      console.log(`input.unassign -> Removed event listener for ${type}`);
    }
  },

  destroy: function() {
    for (const [type, callbacks] of Object.entries(this.eventListeners)) {
      for (const callback of callbacks) {
        window.removeEventListener(type, callback);
      }
    }
    this.eventListeners = {};
    console.log(`input.destroy -> Removed all event listeners`);
  },

  reassign: function() {
    this.destroy();
    this.init?.();
    console.log(`input.reassign -> Reinitialized all input listeners`);
  },

  updateInputMethod: function(method, name = '') {
    const inputMethodDisplay = document.getElementById('input_method');
    if (inputMethodDisplay) {
      inputMethodDisplay.innerText = `Input: ${method}${name ? ' (' + name + ')' : ''}`;
    }
  }
};