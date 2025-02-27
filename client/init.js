input.assign("keydown+shift+f", () => {
  plugin.ui.fullScreen();
});

plugin.preload([
  { id: "auth", ext: "njk" }
]);