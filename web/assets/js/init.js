assets.preload([
    { name: 'gamepad_buttons', path: 'img/icons/gamepad.png' },
    { name: 'female-01', path: 'img/sprites/characters/female-01.png' },
    { name: 'gen1', path: 'img/sheets/gen1.png' },
    { name: 'objectData', path: 'json/objectData.json' },
    { name: 'spritesData', path: 'json/spritesData.json' },
], () => {

    game.objectData = assets.use('objectData');

    input.init();
    game.create();
    game.scene(game.sceneid);

    sprite.create({
        id: game.playerid,
        isPlayer: true,
        animalType: 'female-01'
    });
      
    game.mainSprite = game.sprites[game.playerid];

    plugin.load({ id: 'gamepad_plugin', url: 'gamepad/index.js', drag: false, reload: true });
    plugin.load({ id: 'auth_window', url: 'auth/index.php', drag: true, reload: true });
    plugin.load({ id: 'notif', url: 'notifs/index.js', drag: false, reload: true });
    plugin.load({ id: 'context_menu', url: 'ui/menus/context_menu/index.html', drag: false, reload: true });
    plugin.load({ id: 'ui_overlay_window', url: 'ui/hud/index.php', drag: false, reload: true });

});