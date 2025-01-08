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

    plugin.load({
        id: 'console_window',
        url: 'editor/console/index.php',
        drag: false,
        reload: true,
        onAfterLoad: function () {
            plugin.load({
                id: 'edit_mode_window',
                url: 'editor/index.php',
                drag: false,
                reload: true
            });
        }
    });
});