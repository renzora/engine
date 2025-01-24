input = {
  eventListeners: {},
  listenerMap: {},
  assign: function(type, callback, options = {}) {
    if (type.includes('+')) {
      const parts = type.split('+');
      const mainType = parts[0];
      const subParts = parts.slice(1);
      const autoId = subParts.join('').replace(/\+/g, '');
      const mainKey = subParts[subParts.length - 1].toLowerCase();
      const modifiers = subParts.slice(0, -1);

      const comboListener = (e) => {
        // Check each modifier
        for (let mod of modifiers) {
          mod = mod.toLowerCase();
          if (mod === 'shift' && !e.shiftKey) return;
          if (mod === 'ctrl' && !e.ctrlKey)   return;
          if (mod === 'alt' && !e.altKey)     return;
          if (mod === 'meta' && !e.metaKey)   return;
        }

        if (e.key.toLowerCase() === mainKey) {
          console.log(`input.assign -> Handling "${type}" (id=${autoId})`);
          callback(e);
        }
      };

      if (!this.eventListeners[mainType]) {
        this.eventListeners[mainType] = new Set();
      }
      if (!this.eventListeners[mainType].has(comboListener)) {
        this.eventListeners[mainType].add(comboListener);
        window.addEventListener(mainType, comboListener, options);
        console.log(`input.assign -> Added combo listener for "${type}" (id=${autoId})`);

        this.listenerMap[autoId] = {
          domEvent: mainType,
          actualCallback: comboListener
        };
      }
      return autoId;

    } else if (type.includes('.')) {

      const [mainType, subKey] = type.split('.');
      if (!subKey) return;

      const keyName = subKey.toLowerCase();
      const subId = mainType + keyName;

      const keyListener = (e) => {
        if (e.key.toLowerCase() === keyName) {
          if (["tab", " "].includes(keyName)) {
            e.preventDefault();
          }
          console.log(`input.assign -> Handling ${mainType}.${keyName} (id=${subId})`);
          callback(e);
        }
      };

      if (!this.eventListeners[mainType]) {
        this.eventListeners[mainType] = new Set();
      }
      if (!this.eventListeners[mainType].has(keyListener)) {
        this.eventListeners[mainType].add(keyListener);
        window.addEventListener(mainType, keyListener, options);
        console.log(`input.assign -> Added event listener for ${type} (id=${subId})`);

        this.listenerMap[subId] = {
          domEvent: mainType,
          actualCallback: keyListener
        };
      }
      return subId;

    } else {
      if (!this.eventListeners[type]) {
        this.eventListeners[type] = new Set();
      }
      if (!this.eventListeners[type].has(callback)) {
        this.eventListeners[type].add(callback);
        window.addEventListener(type, callback, options);
        console.log(`input.assign -> Added event listener for ${type}`);
        
        const autoId = type.replace(/[^\w]/g, '') + '_gen';
        this.listenerMap[autoId] = {
          domEvent: type,
          actualCallback: callback
        };
        return autoId;
      }
    }
  },

  destroy: function(id) {
    if (!id) {
      for (const [eventType, callbacks] of Object.entries(this.eventListeners)) {
        for (const cb of callbacks) {
          window.removeEventListener(eventType, cb);
        }
      }
      this.eventListeners = {};
      this.listenerMap = {};
      console.log('input.destroy -> Removed all event listeners');
      return;
    }

    const record = this.listenerMap[id];
    if (!record) {
      console.warn(`input.destroy -> No listener found for id="${id}"`);
      return;
    }

    const { domEvent, actualCallback } = record;
    if (this.eventListeners[domEvent] && this.eventListeners[domEvent].has(actualCallback)) {
      this.eventListeners[domEvent].delete(actualCallback);
      window.removeEventListener(domEvent, actualCallback);
      console.log(`input.destroy -> Removed event listener for id="${id}" (${domEvent})`);
    }
    delete this.listenerMap[id];
  },

  reassign: function() {
    this.destroy();
    if (typeof this.init === 'function') {
      this.init();
    }
    console.log(`input.reassign -> Reinitialized all input listeners`);
  },

  updateInputMethod: function(method, name = '') {
    const inputMethodDisplay = document.getElementById('input_method');
    if (inputMethodDisplay) {
      inputMethodDisplay.innerText = `Input: ${method}${name ? ' (' + name + ')' : ''}`;
    }
  }
};