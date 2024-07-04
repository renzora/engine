var ui = {
  notificationCount: 0,
  activeNotifications: new Map(),

  notif: function(id, message, replace = false) {
      return new Promise(resolve => {
        audio.playAudio("notification", assets.load('notification'), 'sfx', false);
          let container = document.getElementById('notification');
          if (!container) {
              container = document.createElement('div');
              container.id = 'notification';
              container.className = 'fixed z-10 bottom-0 left-1/2 transform -translate-x-1/2';
              document.body.appendChild(container);
          }

          if (this.activeNotifications.has(id)) {
              const existingNotification = this.activeNotifications.get(id);
              if (replace) {
                  existingNotification.innerText = message;

                  // Clear the existing timer and reset it
                  clearTimeout(existingNotification.timer);
                  existingNotification.timer = setTimeout(() => {
                      existingNotification.classList.add('notification-exit');
                      setTimeout(() => {
                          existingNotification.remove();
                          this.notificationCount--;
                          this.activeNotifications.delete(id);

                          if (this.notificationCount === 0) {
                              container.remove();
                          }
                          resolve();
                      }, 1000);
                  }, 3000);
                  return;
              } else {
                  resolve();
                  return;
              }
          }

          const notification = document.createElement('div');
          notification.className = 'notif text-white text-lg px-4 py-2 rounded shadow-md m-2';
          notification.innerText = message; // Use the message parameter
          notification.dataset.id = id; // Assign id to notification element
          container.prepend(notification);

          this.notificationCount++;
          this.activeNotifications.set(id, notification); // Add id to active notifications map

          // Set and store the timer
          notification.timer = setTimeout(() => {
              notification.classList.add('notification-exit');

              setTimeout(() => {
                  notification.remove();
                  this.notificationCount--;
                  this.activeNotifications.delete(id); // Remove id from active notifications map

                  if (this.notificationCount === 0) {
                      container.remove();
                  }
                  resolve();
              }, 1000);
          }, 3000);
      });
  },
  html: function(selectorOrElement, htmlString, action = 'replace') {
    const element = (typeof selectorOrElement === 'string') ? document.querySelector(selectorOrElement) : selectorOrElement;

    if (!element) {
        return;
    }

    switch (action) {
        case 'append':
            element.insertAdjacentHTML('beforeend', htmlString);
            break;
        case 'prepend':
            element.insertAdjacentHTML('afterbegin', htmlString);
            break;
        case 'html':
        default:
            element.innerHTML = htmlString;
            break;
    }

    // Execute scripts from the HTML string
    const tempContainer = document.createElement('div');
    tempContainer.innerHTML = htmlString;
    Array.from(tempContainer.querySelectorAll('script')).forEach(oldScript => {
        const newScript = document.createElement('script');
        if (oldScript.src) {
            // If the script tag has a src attribute, set it on the new script element
            newScript.src = oldScript.src;
            newScript.async = false; // Ensure scripts are executed in order
        } else {
            // If it's an inline script, set its text content
            newScript.textContent = oldScript.textContent;
        }
        // Copy over any other attributes
        Array.from(oldScript.attributes).forEach(attr => newScript.setAttribute(attr.name, attr.value));
        document.body.appendChild(newScript);
        // Remove the script tag after it is executed
        document.body.removeChild(newScript);
    });
},

  unmount: function(id) {
    if (window[id] && typeof window[id].unmount === 'function') {
        window[id].unmount();
    }

    var obj = window[id];

    // Clear properties of the object
    for (var prop in obj) {
        if (obj.hasOwnProperty(prop)) {
            if (typeof obj[prop] === "function") {
                delete obj[prop];
            } else if (Array.isArray(obj[prop])) {
                obj[prop] = [];
            } else if (typeof obj[prop] === "object" && obj[prop] !== null) {
                obj[prop] = {};
            } else {
                obj[prop] = null;
            }
        }
    }
},

  ajax: async function({ url, method = 'GET', data = null, outputType = 'text', success, error }) {
      try {
    
        const queryParams = new URLSearchParams(data).toString();
        const fetchUrl = method === 'GET' && data ? `${url}?${queryParams}` : url;
    
        const init = {
          method: method,
          headers: {}
        };
    
        if (data && method !== 'GET') {
          init.headers['Content-Type'] = 'application/x-www-form-urlencoded';
          init.body = data;
        }
    
        const response = await fetch(fetchUrl, init);
    
        if (!response.ok) { throw new Error('Network response was not ok: ' + response.statusText); }
    
        let responseData;
        switch (outputType) {
          case 'json':
            responseData = await response.json();
            break;
          case 'blob':
            responseData = await response.blob();
            break;
          case 'formData':
            responseData = await response.formData();
            break;
          case 'arrayBuffer':
            responseData = await response.arrayBuffer();
            break;
          default:
            responseData = await response.text();
        }
    
        if (success) success(responseData);
    
      } catch (err) {
        if (error) error(err);
      }
    },

    tabs: {},

    initTabs: function(containerId, defaultTab) {
      const container = document.getElementById(containerId);
      if (!container) return;
  
      const tabButtons = container.querySelectorAll('[data-tab]');
      const tabContents = container.querySelectorAll('[data-tab-content]');
  
      tabButtons.forEach(button => {
        button.addEventListener('click', () => {
          const target = button.getAttribute('data-tab');
          this.showTab(target, tabButtons, tabContents);
          audio.playAudio("click", assets.load('click'), 'sfx');
        });
      });
  
      // Store the initialized tabs and contents
      this.tabs[containerId] = { tabButtons, tabContents };
  
      // Set the default active tab
      const initialTab = defaultTab || tabButtons[0].getAttribute('data-tab');
      this.showTab(initialTab, tabButtons, tabContents);
    },
  
    showTab: function(target, tabButtons, tabContents) {
      tabButtons.forEach(button => {
        button.classList.remove('active');
        if (button.getAttribute('data-tab') === target) {
          button.classList.add('active');
        }
      });
  
      tabContents.forEach(content => {
        content.classList.remove('active');
        if (content.getAttribute('data-tab-content') === target) {
          content.classList.add('active');
        }
      });
    },
  
    destroyTabs: function(containerId) {
      const container = document.getElementById(containerId);
      if (!container || !this.tabs[containerId]) return;
  
      const { tabButtons, tabContents } = this.tabs[containerId];
  
      // Remove event listeners
      tabButtons.forEach(button => {
        button.replaceWith(button.cloneNode(true));
      });
  
      // Clear the tabButtons and tabContents
      tabButtons.forEach(button => button.remove());
      tabContents.forEach(content => content.remove());
  
      // Remove the stored reference
      delete this.tabs[containerId];
    }
};