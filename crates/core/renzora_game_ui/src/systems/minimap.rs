//! Minimap — render-to-texture display.
// The minimap renders a secondary camera to a texture displayed in the UI.
// Camera setup and tracking are project-specific.
// Games should:
// 1. Create a secondary camera with RenderTarget::Image
// 2. Set the camera to follow the player from above
// 3. The MinimapData.zoom can be used to adjust the camera's orthographic size
