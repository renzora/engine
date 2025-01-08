document.addEventListener('DOMContentLoaded', (e) => {

    assets.preload([
        { name: 'gamepad_buttons', path: 'img/icons/gamepad.png' },
        { name: 'female-01', path: 'img/sprites/characters/female-01.png' },
        { name: 'gen1', path: 'img/sheets/gen1.png' },
        { name: 'itemsImg', path: 'img/icons/items.png' },
        { name: 'objectData', path: 'json/objectData.json' },
        { name: 'itemsData', path: 'json/itemsData.json' },
        { name: 'spritesData', path: 'json/spritesData.json' },
        { name: 'fxData', path: 'json/fxData.json' },
    ], () => {

        game.itemsImg = assets.use('itemsImg');
        game.itemsData = assets.use('itemsData');
        game.objectData = assets.use('objectData');
        game.fxData = assets.use('fxData');

        plugin.load({ id: 'main_title_window', url: 'ui/menus/main_menu/index.php', drag: true,reload: true });
        input.init(e);
        game.create({
            
        });
        game.scene(game.sceneid);

    });

});