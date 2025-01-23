input = {
    eventListeners: {},
  
    assign: function(type, callback, options = {}) {
      let [mainType, subKey] = type.split(".");
    
      if (subKey) {
        const keyName = subKey.toLowerCase();
    
        const keyListener = (e) => {
          if (e.key.toLowerCase() === keyName) {
            if (["tab", " "].includes(keyName)) {
              e.preventDefault();
            }
            callback(e);
          }
        };
    
        if (!this.eventListeners[mainType]) {
          this.eventListeners[mainType] = new Set();
        }
        if (!this.eventListeners[mainType].has(keyListener)) {
          this.eventListeners[mainType].add(keyListener);
          window.addEventListener(mainType, keyListener, options);
        }
      } else {
        if (!this.eventListeners[type]) {
          this.eventListeners[type] = new Set();
        }
        if (!this.eventListeners[type].has(callback)) {
          this.eventListeners[type].add(callback);
          window.addEventListener(type, callback, options);
        }
      }
    },
  
    unassign: function(type, callback) {
      if (this.eventListeners[type] && this.eventListeners[type].has(callback)) {
        this.eventListeners[type].delete(callback);
        window.removeEventListener(type, callback);
      }
    },
  
    destroy: function() {
      for (const [type, callbacks] of Object.entries(this.eventListeners)) {
        for (const callback of callbacks) {
          window.removeEventListener(type, callback);
        }
      }
      this.eventListeners = {};
    },
  
    reassign: function() {
      this.destroy();
      this.init();
    }
  };
  