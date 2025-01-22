# Notification plugin for Renzora Engine

```
plugin.load({
    id: 'notif',
    url: 'notifs/index.js',
    drag: false,
    reload: true,
    after: function() {
        notif.show('notification_id', 'change this message to anything you want');
    }
});
```