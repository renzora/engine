window[id] = {
    id: id,
    notificationCount: 0,
    activeNotifications: new Map(),

    start:function() {

    },

    unmount: function() {

    },

    show: function(id, message, replace = false) {
        return new Promise(resolve => {
          audio.playAudio("notification", assets.use('notification'), 'sfx', false);
            let container = document.getElementById('notification');
            if (!container) {
                container = document.createElement('div');
                container.id = 'notification';
                container.className = 'fixed z-10 top-0 left-1/2 transform -translate-x-1/2';
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
    }
};