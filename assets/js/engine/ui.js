var ui = {
    notificationCount: 0,
    notif: function(message) {
        return new Promise(resolve => {
            let container = document.getElementById('notification');
            if(!container) {
                container = document.createElement('div');
                container.id = 'notification';
                container.className = 'fixed z-10 bottom-0 left-1/2 transform -translate-x-1/2';
                document.body.appendChild(container);
            }
    
            const notification = document.createElement('div');
            notification.className = 'notif text-white text-lg px-4 py-2 rounded shadow-md m-2';
            notification.innerText = message;
            container.prepend(notification);
    
            this.notificationCount++;
    
            setTimeout(() => {
                notification.classList.add('notification-exit');
    
                setTimeout(() => {
                    notification.remove();
                    this.notificationCount--;
    
                    if(this.notificationCount === 0) {
                        container.remove();
                    }
                    resolve();
                }, 1000);
            }, 3000);
        });
    },
    html: function(selectorOrElement, htmlString, action = 'replace') {
        const element = (typeof selectorOrElement === 'string') ? document.querySelector(selectorOrElement) : selectorOrElement;
      
        if(!element) { return; }
      
        switch(action) {
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
      
        const tempContainer = document.createElement('div');
        tempContainer.innerHTML = htmlString;
        Array.from(tempContainer.querySelectorAll("script")).forEach(oldScript => {
            const newScript = document.createElement("script");
            Array.from(oldScript.attributes).forEach(attr => newScript.setAttribute(attr.name, attr.value));
            newScript.textContent = oldScript.textContent;
            document.body.appendChild(newScript);
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
      }
}