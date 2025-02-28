// This is your startup file. Add all your plugins, preload assets and do any engine configurations here.
assets.preload(
  [
    { name: "female-01", path: "assets/img/sprites/characters/female-01.png", noCache: true },
    { name: "objectData", path: "assets/json/objectData.json", noCache: true },
    { name: "spriteData", path: "assets/json/spritesData.json", noCache: true },
  ],
  () => {
    input.assign("keydown+shift+e", () => {
      plugin.load("console_window", {
        path: "editor",
        ext: "njk",
        drag: false,
        reload: true,
        before: function () {
          plugin.hideAll();
        },
        after: function () {
          plugin.load("editor_window", { path: "editor", ext: "njk" });
        },
      });
    });

    input.assign("keydown+shift+f", () => {
      plugin.ui.fullScreen();
    });

    plugin.preload([
      { id: "time", path: "core" },
      { id: "notif", path: "core", ext: "html", after: function () {
          notif.show("remove_messages", "edit init.js to remove these messages", "danger");
          notif.show("access_editor", "press shift + e to access editor");
        },
      },
      { id: "auth", ext: "njk" },
      { id: 'lighting', path: 'core' },
      { id: 'collision', path: 'core' },
      { id: 'pathfinding' },
      { id: 'debug', path: 'core', ext: 'html' },
      { id: 'ui', path: 'core' },
      { id: 'gamepad' },
      { id: 'actions' }
    ]);

    const playerSprite = sprite.create({
      id: "player1",
      isPlayer: true,
      speed: 85,
      topSpeed: 85,
      currentAnimation: "idle",
      type: "female-01",
    });

    game.create({
      objectData: assets.use("objectData"),
      spriteData: assets.use("spriteData"),
      player: playerSprite,
      after: function () {
        game.scene(
          localStorage.getItem("sceneid") || "678ec2d7433aae2deee168ee"
        );
        sprite.init();
        plugin.time.hours = 6;
      },
    });
  }
);