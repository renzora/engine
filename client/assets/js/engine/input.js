input = {
  eventListeners: {},
  listenerMap: {},
  
  assign(type, callback, options = {}, targetElement = window) {
      const createId = (type, subParts) => subParts ? subParts.join('').replace(/\+/g, '') : type.replace(/[^\w]/g, '') + '_gen';

      const addListener = (eventType, listener, id, element = targetElement) => {
          if (!this.eventListeners[eventType]) {
              this.eventListeners[eventType] = new Set();
          }
          if (!this.eventListeners[eventType].has(listener)) {
              this.eventListeners[eventType].add(listener);
              element.addEventListener(eventType, listener, options);
              this.listenerMap[id] = { domEvent: eventType, actualCallback: listener, element };
              return id;
          }
      };

      if (type.includes('+')) {
          const [mainType, ...subParts] = type.split('+');
          const mainKey = subParts[subParts.length - 1].toLowerCase();
          const modifiers = subParts.slice(0, -1);
          const id = createId(type, subParts);

          const comboListener = (e) => {
              const modCheck = {
                  'shift': e.shiftKey,
                  'ctrl': e.ctrlKey,
                  'alt': e.altKey,
                  'meta': e.metaKey
              };
              if (modifiers.some(mod => !modCheck[mod.toLowerCase()])) return;
              if (e.key.toLowerCase() === mainKey) callback(e);
          };

          return addListener(mainType, comboListener, id);
      } 
      
      if (type.includes('.')) {
          const [mainType, subKey] = type.split('.');
          if (!subKey) return;
          
          const id = mainType + subKey.toLowerCase();
          const keyListener = (e) => {
              if (e.key.toLowerCase() === subKey.toLowerCase()) {
                  if (["tab", " "].includes(subKey.toLowerCase())) e.preventDefault();
                  callback(e);
              }
          };

          return addListener(mainType, keyListener, id);
      }

      return addListener(type, callback, createId(type));
  },

  destroy(id) {
      if (!id) {
          Object.entries(this.eventListeners).forEach(([type, callbacks]) => {
              callbacks.forEach(cb => {
                  const record = Object.values(this.listenerMap).find(r => r.actualCallback === cb);
                  (record?.element || window).removeEventListener(type, cb);
              });
          });
          this.eventListeners = {};
          this.listenerMap = {};
          return;
      }

      const record = this.listenerMap[id];
      if (record && this.eventListeners[record.domEvent]?.has(record.actualCallback)) {
          this.eventListeners[record.domEvent].delete(record.actualCallback);
          record.element.removeEventListener(record.domEvent, record.actualCallback);
          delete this.listenerMap[id];
      }
  },

  reassign() {
      this.destroy();
      if (this.init) this.init();
  },

  updateInputMethod(method, name = '') {
      const display = document.getElementById('input_method');
      if (display) display.innerText = `Input: ${method}${name ? ` (${name})` : ''}`;
  }
};