assets.preload([
    { name: 'gamepad_buttons', path: 'img/icons/gamepad.png' },
    { name: 'female-01', path: 'img/sprites/characters/female-01.png' },
    { name: 'gen1', path: 'img/sheets/gen1.png' },
    { name: 'objectData', path: 'json/objectData.json' },
    { name: 'spritesData', path: 'json/spritesData.json' },
], () => {

    game.objectData = assets.use('objectData');

    game.create();

    audio.createChannel('music', localStorage.getItem('music-volume') || audio.defaultVolume);
    audio.setVolume('music', localStorage.getItem('music-volume') || 0.05);
    audio.createChannel('sfx', localStorage.getItem('sfx-volume') || audio.defaultVolume);
    audio.createChannel('ambience', localStorage.getItem('ambience-volume') || 0.5);
    
    game.scene(game.sceneid);

    sprite.create({
        id: game.playerid,
        isPlayer: true,
        speed: 100,
        animalType: 'female-01',
        canShoot: true,
        targetAim: true
    });
      
    game.mainSprite = game.sprites[game.playerid];

    plugin.load({ id: 'gamepad_plugin', url: 'gamepad/index.js', drag: false, reload: true });
    plugin.load({ id: 'auth_window', url: 'auth/index.php', drag: true, reload: true });
    plugin.load({ id: 'notif', url: 'notifs/index.js', drag: false, reload: true });
    plugin.load({ id: 'context_menu', url: 'ui/menus/context_menu/index.html', drag: false, reload: true });
    plugin.load({ id: 'ui_overlay_window', url: 'ui/hud/index.php', drag: false, reload: true });
    plugin.load({
        id: 'weather_plugin',
        url: 'effects/weather/index.js',
        reload: true,
        after: function() {
            weather_plugin.snow.active = true;
        }
    });

    plugin.load({
        id: 'console_window',
        url: 'editor/console/index.php',
        drag: false,
        reload: true,
        after: function () {
          plugin.load({
              id: 'edit_mode_window',
              url: 'editor/main/index.php',
              drag: false,
              reload: true
          });
        }
      });

});