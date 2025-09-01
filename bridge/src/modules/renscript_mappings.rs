pub fn get_api_method_mappings() -> Vec<(String, String)> {
    vec![
        // === CORE FUNCTIONS ===
        ("log".to_string(), "log".to_string()),
        ("time".to_string(), "time".to_string()),
        ("addTag".to_string(), "addTag".to_string()),
        ("removeTag".to_string(), "removeTag".to_string()),
        ("hasTag".to_string(), "hasTag".to_string()),
        ("getTags".to_string(), "getTags".to_string()),

        // === TRANSFORM FUNCTIONS ===
        ("position".to_string(), "getPosition".to_string()),
        ("setPosition".to_string(), "setPosition".to_string()),
        ("rotation".to_string(), "getRotation".to_string()),
        ("setRotation".to_string(), "setRotation".to_string()),
        ("setScale".to_string(), "setScale".to_string()),
        ("move".to_string(), "moveBy".to_string()),
        ("rotate".to_string(), "rotateBy".to_string()),

        // === COLOR & MATERIAL FUNCTIONS ===
        ("color".to_string(), "setColor".to_string()),
        ("setColor".to_string(), "setColor".to_string()),
        ("setAlpha".to_string(), "setAlpha".to_string()),
        ("diffuseColor".to_string(), "setDiffuseColor".to_string()),
        ("specularColor".to_string(), "setSpecularColor".to_string()),
        ("emissiveColor".to_string(), "setEmissiveColor".to_string()),
        ("ambientColor".to_string(), "setAmbientColor".to_string()),
        ("specularPower".to_string(), "setSpecularPower".to_string()),
        ("materialProperty".to_string(), "setMaterialProperty".to_string()),

        // === INPUT FUNCTIONS ===
        ("isKeyPressed".to_string(), "isKeyPressed".to_string()),
        ("isCtrlPressed".to_string(), "isCtrlPressed".to_string()),
        ("isShiftPressed".to_string(), "isShiftPressed".to_string()),
        ("isKeyComboPressed".to_string(), "isKeyComboPressed".to_string()),
        ("getPressedKeys".to_string(), "getPressedKeys".to_string()),
        ("mousePosition".to_string(), "getMousePosition".to_string()),
        ("mouseX".to_string(), "getMouseX".to_string()),
        ("mouseY".to_string(), "getMouseY".to_string()),
        ("isLeftMouse".to_string(), "isLeftMouseButtonPressed".to_string()),
        ("isRightMouse".to_string(), "isRightMouseButtonPressed".to_string()),
        ("isMiddleMouse".to_string(), "isMiddleMouseButtonPressed".to_string()),
        ("pick".to_string(), "pickObject".to_string()),
        ("pickObjects".to_string(), "pickObjects".to_string()),
        ("pointerLock".to_string(), "requestPointerLock".to_string()),
        ("exitPointerLock".to_string(), "exitPointerLock".to_string()),

        // === GAMEPAD FUNCTIONS ===
        ("isGamepadConnected".to_string(), "isGamepadConnected".to_string()),
        ("getGamepad".to_string(), "getGamepad".to_string()),
        ("leftX".to_string(), "getLeftStickX".to_string()),
        ("leftY".to_string(), "getLeftStickY".to_string()),
        ("rightX".to_string(), "getRightStickX".to_string()),
        ("rightY".to_string(), "getRightStickY".to_string()),
        ("button".to_string(), "isGamepadButtonPressed".to_string()),
        ("leftTrigger".to_string(), "getLeftTrigger".to_string()),
        ("rightTrigger".to_string(), "getRightTrigger".to_string()),
        ("isButtonA".to_string(), "isGamepadButtonA".to_string()),
        ("isButtonB".to_string(), "isGamepadButtonB".to_string()),
        ("isButtonX".to_string(), "isGamepadButtonX".to_string()),
        ("isButtonY".to_string(), "isGamepadButtonY".to_string()),
        ("leftStickDeadzone".to_string(), "getLeftStickWithDeadzone".to_string()),
        ("rightStickDeadzone".to_string(), "getRightStickWithDeadzone".to_string()),
        ("vibrate".to_string(), "vibrateGamepad".to_string()),

        // === PHYSICS FUNCTIONS ===
        ("physics".to_string(), "enablePhysics".to_string()),
        ("physicsAggregate".to_string(), "setPhysicsImpostor".to_string()),
        ("updatePhysics".to_string(), "updatePhysics".to_string()),
        ("removePhysicsAggregate".to_string(), "removePhysicsImpostor".to_string()),
        ("impulse".to_string(), "applyImpulse".to_string()),
        ("force".to_string(), "applyForce".to_string()),
        ("linearVelocity".to_string(), "setLinearVelocity".to_string()),
        ("getLinearVelocity".to_string(), "getLinearVelocity".to_string()),
        ("angularVelocity".to_string(), "setAngularVelocity".to_string()),

        // === CAMERA FUNCTIONS ===
        ("cameraPosition".to_string(), "setCameraPosition".to_string()),
        ("cameraTarget".to_string(), "setCameraTarget".to_string()),
        ("cameraRadius".to_string(), "setCameraRadius".to_string()),
        ("orbitCamera".to_string(), "orbitCamera".to_string()),
        ("isInCameraView".to_string(), "isInCameraView".to_string()),

        // === SCENE QUERY FUNCTIONS ===
        ("findByName".to_string(), "findObjectByName".to_string()),
        ("findByTag".to_string(), "findObjectsByTag".to_string()),
        ("getAllMeshes".to_string(), "getAllMeshes".to_string()),
        ("getAllLights".to_string(), "getAllLights".to_string()),
        ("getAllCameras".to_string(), "getAllCameras".to_string()),
        ("getInRadius".to_string(), "getObjectsInRadius".to_string()),
        ("getInBox".to_string(), "getObjectsInBox".to_string()),
        ("getClosest".to_string(), "getClosestObject".to_string()),
        ("raycast".to_string(), "raycast".to_string()),
        ("multiRaycast".to_string(), "multiRaycast".to_string()),
        ("intersectsMesh".to_string(), "intersectsMesh".to_string()),
        ("intersectsPoint".to_string(), "intersectsPoint".to_string()),
        ("getBoundingInfo".to_string(), "getBoundingInfo".to_string()),

        // === ANIMATION FUNCTIONS ===
        ("animate".to_string(), "animate".to_string()),
        ("animatePosition".to_string(), "animatePosition".to_string()),
        ("animateRotation".to_string(), "animateRotation".to_string()),
        ("animateColor".to_string(), "animateColor".to_string()),
        ("animateAlpha".to_string(), "animateAlpha".to_string()),
        ("animateTo".to_string(), "animateTo".to_string()),
        ("stopAnimation".to_string(), "stopAnimation".to_string()),
        ("animationGroup".to_string(), "createAnimationGroup".to_string()),
        ("playAnimationGroup".to_string(), "playAnimationGroup".to_string()),
        ("stopAnimationGroup".to_string(), "stopAnimationGroup".to_string()),
        ("keyframeAnimation".to_string(), "createKeyframeAnimation".to_string()),
        ("blendAnimations".to_string(), "blendAnimations".to_string()),
        ("onAnimationComplete".to_string(), "onAnimationComplete".to_string()),
        ("isAnimating".to_string(), "isAnimating".to_string()),
        ("getAnimationProgress".to_string(), "getAnimationProgress".to_string()),
        ("playAnimationByName".to_string(), "playAnimationByName".to_string()),
        ("animationSpeed".to_string(), "setAnimationSpeed".to_string()),
        ("isAnimationPlaying".to_string(), "isAnimationPlaying".to_string()),
        ("getCurrentAnimation".to_string(), "getCurrentAnimation".to_string()),
        ("hasSkeleton".to_string(), "hasSkeleton".to_string()),
        ("getBoneCount".to_string(), "getBoneCount".to_string()),

        // === UTILITY FUNCTIONS ===
        ("random".to_string(), "random".to_string()),
        ("randomRange".to_string(), "randomRange".to_string()),
        ("clamp".to_string(), "clamp".to_string()),
        ("lerp".to_string(), "lerp".to_string()),
        ("distance".to_string(), "distance".to_string()),
        ("normalize".to_string(), "normalize".to_string()),
        ("sin".to_string(), "sin".to_string()),
        ("cos".to_string(), "cos".to_string()),
        ("sqrt".to_string(), "sqrt".to_string()),
        ("abs".to_string(), "abs".to_string()),
        ("floor".to_string(), "floor".to_string()),
        ("atan2".to_string(), "atan2".to_string()),

        // === MATERIAL CREATION ===
        ("waterMaterial".to_string(), "createWaterMaterial".to_string()),
        ("standardMaterial".to_string(), "createStandardMaterial".to_string()),

        // === OBJECT MANAGEMENT ===
        ("dispose".to_string(), "disposeObject".to_string()),
        ("clone".to_string(), "cloneObject".to_string()),
        ("getMetadata".to_string(), "getMetadata".to_string()),
        ("setMetadata".to_string(), "setMetadata".to_string()),
        ("hasMetadata".to_string(), "hasMetadata".to_string()),

        // === EVENT HANDLERS ===
        ("onKeyDown".to_string(), "onKeyDown".to_string()),
        ("onKeyUp".to_string(), "onKeyUp".to_string()),
        ("onMouseDown".to_string(), "onMouseDown".to_string()),
        ("onMouseUp".to_string(), "onMouseUp".to_string()),

    ]
}