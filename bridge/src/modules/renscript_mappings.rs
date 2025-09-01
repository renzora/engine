pub fn get_api_method_mappings() -> Vec<(String, String)> {
    vec![
        // === CORE TRANSFORM & UTILITY ===
        ("log".to_string(), "log".to_string()),
        ("position".to_string(), "getPosition".to_string()),
        ("getPosition".to_string(), "getPosition".to_string()),
        ("setPosition".to_string(), "setPosition".to_string()),
        ("worldPosition".to_string(), "getWorldPosition".to_string()),
        ("rotation".to_string(), "getRotation".to_string()),
        ("getRotation".to_string(), "getRotation".to_string()),
        ("setRotation".to_string(), "setRotation".to_string()),
        ("worldRotation".to_string(), "getWorldRotationQuaternion".to_string()),
        ("scale".to_string(), "getScale".to_string()),
        ("getScale".to_string(), "getScale".to_string()),
        ("setScale".to_string(), "setScale".to_string()),
        ("rotate".to_string(), "rotateBy".to_string()),
        ("move".to_string(), "moveBy".to_string()),
        ("moveTo".to_string(), "setPosition".to_string()),
        ("lookAt".to_string(), "lookAt".to_string()),
        ("visible".to_string(), "setVisible".to_string()),
        ("isVisible".to_string(), "isVisible".to_string()),
        ("enabled".to_string(), "setEnabled".to_string()),
        ("isEnabled".to_string(), "isEnabled".to_string()),
        ("name".to_string(), "getName".to_string()),
        ("getName".to_string(), "getName".to_string()),
        ("setName".to_string(), "setName".to_string()),
        ("id".to_string(), "getId".to_string()),

        // === TAGS & METADATA ===
        ("addTag".to_string(), "addTag".to_string()),
        ("removeTag".to_string(), "removeTag".to_string()),
        ("hasTag".to_string(), "hasTag".to_string()),
        ("tags".to_string(), "getTags".to_string()),
        ("getTags".to_string(), "getTags".to_string()),
        ("metadata".to_string(), "getMetadata".to_string()),
        ("getMetadata".to_string(), "getMetadata".to_string()),
        ("setMetadata".to_string(), "setMetadata".to_string()),
        ("hasMetadata".to_string(), "hasMetadata".to_string()),
        ("removeMetadata".to_string(), "removeMetadata".to_string()),

        // === TIME & UTILITY ===
        ("getTime".to_string(), "getTime".to_string()),
        ("time".to_string(), "time".to_string()), // Short alias for time (delta time)
        ("getDeltaTime".to_string(), "getDeltaTime".to_string()),
        ("deltaTime".to_string(), "getDeltaTime".to_string()), // Short alias for getDeltaTime
        ("random".to_string(), "random".to_string()),
        ("randomRange".to_string(), "randomRange".to_string()),
        ("clamp".to_string(), "clamp".to_string()),
        ("lerp".to_string(), "lerp".to_string()),
        ("distance".to_string(), "distance".to_string()),
        ("normalize".to_string(), "normalize".to_string()),
        ("dot".to_string(), "dot".to_string()),
        ("cross".to_string(), "cross".to_string()),
        ("toRadians".to_string(), "toRadians".to_string()),
        ("toDegrees".to_string(), "toDegrees".to_string()),

        // === MATERIAL & COLOR ===
        ("setColor".to_string(), "setColor".to_string()),
        ("getColor".to_string(), "getColor".to_string()),
        ("color".to_string(), "getColor".to_string()), // Short alias for getColor
        ("setAlpha".to_string(), "setAlpha".to_string()),
        ("getAlpha".to_string(), "getAlpha".to_string()),
        ("alpha".to_string(), "getAlpha".to_string()), // Short alias for getAlpha
        ("diffuseColor".to_string(), "setDiffuseColor".to_string()),
        ("specularColor".to_string(), "setSpecularColor".to_string()),
        ("emissiveColor".to_string(), "setEmissiveColor".to_string()),
        ("ambientColor".to_string(), "setAmbientColor".to_string()),
        ("getEmissiveColor".to_string(), "getEmissiveColor".to_string()),
        ("emissiveColor".to_string(), "getEmissiveColor".to_string()), // Short alias for getEmissiveColor
        ("specularPower".to_string(), "setSpecularPower".to_string()),
        ("materialProperty".to_string(), "setMaterialProperty".to_string()),
        ("getMaterialProperty".to_string(), "getMaterialProperty".to_string()),
        ("materialProperty".to_string(), "getMaterialProperty".to_string()), // Short alias for getMaterialProperty

        // === TEXTURE SYSTEM ===
        ("texture".to_string(), "setTexture".to_string()),
        ("diffuseTexture".to_string(), "setDiffuseTexture".to_string()),
        ("normalTexture".to_string(), "setNormalTexture".to_string()),
        ("emissiveTexture".to_string(), "setEmissiveTexture".to_string()),
        ("specularTexture".to_string(), "setSpecularTexture".to_string()),
        ("ambientTexture".to_string(), "setAmbientTexture".to_string()),
        ("opacityTexture".to_string(), "setOpacityTexture".to_string()),
        ("reflectionTexture".to_string(), "setReflectionTexture".to_string()),
        ("refractionTexture".to_string(), "setRefractionTexture".to_string()),
        ("lightmapTexture".to_string(), "setLightmapTexture".to_string()),
        ("metallicTexture".to_string(), "setMetallicTexture".to_string()),
        ("roughnessTexture".to_string(), "setRoughnessTexture".to_string()),
        ("microRoughnessTexture".to_string(), "setMicroRoughnessTexture".to_string()),
        ("displacementTexture".to_string(), "setDisplacementTexture".to_string()),
        ("detailTexture".to_string(), "setDetailTexture".to_string()),

        // === MATERIAL RENDERING ===
        ("backFaceCulling".to_string(), "setBackFaceCulling".to_string()),
        ("disableLighting".to_string(), "setDisableLighting".to_string()),
        ("wireframe".to_string(), "setWireframe".to_string()),
        ("pointsCloud".to_string(), "setPointsCloud".to_string()),
        ("fillMode".to_string(), "setFillMode".to_string()),
        ("invertNormalMapX".to_string(), "setInvertNormalMapX".to_string()),
        ("invertNormalMapY".to_string(), "setInvertNormalMapY".to_string()),
        ("bumpLevel".to_string(), "setBumpLevel".to_string()),
        ("parallaxScaleBias".to_string(), "setParallaxScaleBias".to_string()),
        ("indexOfRefraction".to_string(), "setIndexOfRefraction".to_string()),
        ("fresnelParameters".to_string(), "setFresnelParameters".to_string()),
        ("standardMaterial".to_string(), "createStandardMaterial".to_string()),
        ("pbrMaterial".to_string(), "createPBRMaterial".to_string()),
        ("dynamicTexture".to_string(), "createDynamicTexture".to_string()),
        ("renderTargetTexture".to_string(), "createRenderTargetTexture".to_string()),

        // === ANIMATION SYSTEM ===
        ("animate".to_string(), "animate".to_string()),
        ("stopAnimation".to_string(), "stopAnimation".to_string()),
        ("pauseAnimation".to_string(), "pauseAnimation".to_string()),
        ("resumeAnimation".to_string(), "resumeAnimation".to_string()),
        ("animatePosition".to_string(), "animatePosition".to_string()),
        ("animateRotation".to_string(), "animateRotation".to_string()),
        ("animateScale".to_string(), "animateScale".to_string()),
        ("animateColor".to_string(), "animateColor".to_string()),
        ("animateAlpha".to_string(), "animateAlpha".to_string()),
        ("animateTo".to_string(), "animateTo".to_string()),
        ("animationGroup".to_string(), "createAnimationGroup".to_string()),
        ("addToAnimationGroup".to_string(), "addToAnimationGroup".to_string()),
        ("playAnimationGroup".to_string(), "playAnimationGroup".to_string()),
        ("stopAnimationGroup".to_string(), "stopAnimationGroup".to_string()),
        ("keyframeAnimation".to_string(), "createKeyframeAnimation".to_string()),
        ("blendAnimations".to_string(), "blendAnimations".to_string()),
        ("onAnimationComplete".to_string(), "onAnimationComplete".to_string()),
        ("onAnimationLoop".to_string(), "onAnimationLoop".to_string()),
        ("animationSpeed".to_string(), "setAnimationSpeed".to_string()),
        ("getAnimationSpeed".to_string(), "getAnimationSpeed".to_string()),
        ("animationSpeed".to_string(), "getAnimationSpeed".to_string()), // Short alias for getAnimationSpeed
        ("isAnimating".to_string(), "isAnimating".to_string()),
        ("getActiveAnimations".to_string(), "getActiveAnimations".to_string()),
        ("activeAnimations".to_string(), "getActiveAnimations".to_string()), // Short alias for getActiveAnimations
        ("getAnimationProgress".to_string(), "getAnimationProgress".to_string()),
        ("animationProgress".to_string(), "getAnimationProgress".to_string()), // Short alias for getAnimationProgress

        // === SKELETON & BONE ANIMATION ===
        ("hasSkeleton".to_string(), "hasSkeleton".to_string()),
        ("getSkeleton".to_string(), "getSkeleton".to_string()),
        ("skeleton".to_string(), "getSkeleton".to_string()), // Short alias for getSkeleton
        ("getBoneCount".to_string(), "getBoneCount".to_string()),
        ("boneCount".to_string(), "getBoneCount".to_string()), // Short alias for getBoneCount
        ("getBone".to_string(), "getBone".to_string()),
        ("bone".to_string(), "getBone".to_string()), // Short alias for getBone
        ("getBoneByName".to_string(), "getBoneByName".to_string()),
        ("boneByName".to_string(), "getBoneByName".to_string()), // Short alias for getBoneByName
        ("bonePosition".to_string(), "setBonePosition".to_string()),
        ("boneRotation".to_string(), "setBoneRotation".to_string()),
        ("getBonePosition".to_string(), "getBonePosition".to_string()),
        ("bonePosition".to_string(), "getBonePosition".to_string()), // Short alias for getBonePosition
        ("getBoneRotation".to_string(), "getBoneRotation".to_string()),
        ("boneRotation".to_string(), "getBoneRotation".to_string()), // Short alias for getBoneRotation

        // Animation range/clip management
        ("playAnimationRange".to_string(), "playAnimationByName".to_string()),
        ("stopAnimationRange".to_string(), "stopAnimationRange".to_string()),
        ("animationRange".to_string(), "setAnimationRange".to_string()),
        ("getAnimationRanges".to_string(), "animation_getAllAnimations".to_string()),

        // Character animation states
        ("walkAnimation".to_string(), "playWalkAnimation".to_string()),
        ("runAnimation".to_string(), "playRunAnimation".to_string()),
        ("idleAnimation".to_string(), "playIdleAnimation".to_string()),
        ("jumpAnimation".to_string(), "playJumpAnimation".to_string()),
        ("crouchAnimation".to_string(), "playCrouchAnimation".to_string()),
        ("customAnimation".to_string(), "playCustomAnimation".to_string()),

        // Animation blending
        ("animationWeight".to_string(), "setAnimationWeight".to_string()),
        ("blendToAnimation".to_string(), "blendToAnimation".to_string()),
        ("crossfadeAnimation".to_string(), "crossfadeAnimation".to_string()),

        // Animation state queries
        ("isAnimationPlaying".to_string(), "isAnimationPlaying".to_string()),
        ("getCurrentAnimation".to_string(), "getCurrentAnimation".to_string()),
        ("currentAnimation".to_string(), "getCurrentAnimation".to_string()), // Short alias for getCurrentAnimation
        ("getAnimationTime".to_string(), "getAnimationTime".to_string()),
        ("animationTime".to_string(), "getAnimationTime".to_string()), // Short alias for getAnimationTime
        ("animationTime".to_string(), "setAnimationTime".to_string()),

        // Basic animation creation
        ("createAnimation".to_string(), "animation_createAnimation".to_string()),
        ("createVectorAnimation".to_string(), "animation_createVectorAnimation".to_string()),
        ("createColorAnimation".to_string(), "animation_createColorAnimation".to_string()),
        ("createQuaternionAnimation".to_string(), "animation_createQuaternionAnimation".to_string()),

        // Animation keyframes
        ("addAnimationKeys".to_string(), "animation_addAnimationKeys".to_string()),
        ("parseAnimationValue".to_string(), "animation_parseAnimationValue".to_string()),

        // Animation playback
        ("playAnimation".to_string(), "animation_playAnimation".to_string()),
        ("stopAnimation".to_string(), "animation_stopAnimation".to_string()),
        ("pauseAnimation".to_string(), "animation_pauseAnimation".to_string()),
        ("restartAnimation".to_string(), "animation_restartAnimation".to_string()),

        // Easing functions
        ("bezierEase".to_string(), "animation_createBezierEase".to_string()),
        ("circleEase".to_string(), "animation_createCircleEase".to_string()),
        ("backEase".to_string(), "animation_createBackEase".to_string()),
        ("bounceEase".to_string(), "animation_createBounceEase".to_string()),
        ("elasticEase".to_string(), "animation_createElasticEase".to_string()),
        ("exponentialEase".to_string(), "animation_createExponentialEase".to_string()),
        ("powerEase".to_string(), "animation_createPowerEase".to_string()),
        ("easingMode".to_string(), "animation_setEasingMode".to_string()),

        // Animation groups
        ("animationGroup".to_string(), "animation_createAnimationGroup".to_string()),
        ("addAnimationToGroup".to_string(), "animation_addAnimationToGroup".to_string()),
        ("playAnimationGroup".to_string(), "animation_playAnimationGroup".to_string()),
        ("stopAnimationGroup".to_string(), "animation_stopAnimationGroup".to_string()),
        ("pauseAnimationGroup".to_string(), "animation_pauseAnimationGroup".to_string()),
        ("resetAnimationGroup".to_string(), "animation_resetAnimationGroup".to_string()),

        // Skeleton animation
        ("skeleton".to_string(), "animation_createSkeleton".to_string()),
        ("playSkeletonAnimation".to_string(), "animation_playSkeletonAnimation".to_string()),
        ("stopSkeletonAnimation".to_string(), "animation_stopSkeletonAnimation".to_string()),
        ("animationRange".to_string(), "animation_createAnimationRange".to_string()),
        ("deleteAnimationRange".to_string(), "animation_deleteAnimationRange".to_string()),
        ("getSkeletonAnimationRanges".to_string(), "animation_getSkeletonAnimationRanges".to_string()),

        // Bone manipulation
        ("getBoneByName".to_string(), "animation_getBoneByName".to_string()),
        ("boneTransform".to_string(), "animation_setBoneTransform".to_string()),
        ("getBoneWorldMatrix".to_string(), "animation_getBoneWorldMatrix".to_string()),
        ("attachMeshToBone".to_string(), "animation_attachMeshToBone".to_string()),

        // Morph target animation
        ("morphTargetManager".to_string(), "animation_createMorphTargetManager".to_string()),
        ("addMorphTarget".to_string(), "animation_addMorphTarget".to_string()),
        ("morphTargetInfluence".to_string(), "animation_setMorphTargetInfluence".to_string()),
        ("animateMorphTarget".to_string(), "animation_animateMorphTarget".to_string()),

        // Advanced animation features
        ("blendAnimations".to_string(), "animation_blendAnimations".to_string()),
        ("animateAlongPath".to_string(), "animation_animateAlongPath".to_string()),
        ("animateRotationAroundAxis".to_string(), "animation_animateRotationAroundAxis".to_string()),
        ("animateOpacity".to_string(), "animation_animateOpacity".to_string()),

        // Animation weight & blending
        ("animationWeight".to_string(), "animation_setAnimationWeight".to_string()),
        ("blendToAnimation".to_string(), "animation_blendToAnimation".to_string()),

        // Animation events
        ("addAnimationEvent".to_string(), "animation_addAnimationEvent".to_string()),
        ("removeAnimationEvents".to_string(), "animation_removeAnimationEvents".to_string()),

        // Animation utilities
        ("getAnimationProgress".to_string(), "animation_getAnimationProgress".to_string()),
        ("isAnimationPlaying".to_string(), "animation_isAnimationPlaying".to_string()),
        ("getAllAnimations".to_string(), "animation_getAllAnimations".to_string()),

        // Physics animation
        ("animateWithPhysics".to_string(), "animation_animateWithPhysics".to_string()),

        // Animation curves
        ("animationCurve".to_string(), "animation_createAnimationCurve".to_string()),
        ("getCurvePoint".to_string(), "animation_getCurvePoint".to_string()),
        ("getCurveTangent".to_string(), "animation_getCurveTangent".to_string()),

        // Smart animation player
        ("playAnimationByName".to_string(), "animation_playAnimationByName".to_string()),

        // Animation info
        ("getAnimationInfo".to_string(), "animation_getAnimationInfo".to_string()),

        // === SCENE QUERIES ===
        ("findByName".to_string(), "findObjectByName".to_string()),
        ("findObjectsByName".to_string(), "findObjectsByName".to_string()),
        ("findByTag".to_string(), "findObjectsByTag".to_string()),
        ("findWithTag".to_string(), "findObjectsWithTag".to_string()),
        ("getAllMeshes".to_string(), "getAllMeshes".to_string()),
        ("getAllLights".to_string(), "getAllLights".to_string()),
        ("getAllCameras".to_string(), "getAllCameras".to_string()),

        // === CAMERA CONTROL ===
        ("getActiveCamera".to_string(), "getActiveCamera".to_string()),
        ("activeCamera".to_string(), "getActiveCamera".to_string()), // Short alias for getActiveCamera
        ("cameraPosition".to_string(), "setCameraPosition".to_string()),
        ("getCameraPosition".to_string(), "getCameraPosition".to_string()),
        ("cameraTarget".to_string(), "setCameraTarget".to_string()),
        ("getCameraTarget".to_string(), "getCameraTarget".to_string()),
        ("cameraTarget".to_string(), "getCameraTarget".to_string()), // Short alias for getCameraTarget
        ("cameraRotation".to_string(), "setCameraRotation".to_string()),
        ("getCameraRotation".to_string(), "getCameraRotation".to_string()),
        ("cameraRotation".to_string(), "getCameraRotation".to_string()), // Short alias for getCameraRotation

        // === RAYCASTING & PICKING ===
        ("raycast".to_string(), "raycast".to_string()),
        ("cameraRaycast".to_string(), "raycastFromCamera".to_string()),
        ("multiRaycast".to_string(), "multiRaycast".to_string()),
        ("pick".to_string(), "pickObject".to_string()),
        ("pickObjects".to_string(), "pickObjects".to_string()),

        // === SPATIAL QUERIES ===
        ("getInRadius".to_string(), "getObjectsInRadius".to_string()),
        ("getInBox".to_string(), "getObjectsInBox".to_string()),
        ("getClosest".to_string(), "getClosestObject".to_string()),
        ("intersectsMesh".to_string(), "intersectsMesh".to_string()),
        ("intersectsPoint".to_string(), "intersectsPoint".to_string()),
        ("getBoundingInfo".to_string(), "getBoundingInfo".to_string()),
        ("boundingInfo".to_string(), "getBoundingInfo".to_string()), // Short alias for getBoundingInfo

        // === OBJECT MANAGEMENT ===
        ("dispose".to_string(), "disposeObject".to_string()),
        ("clone".to_string(), "cloneObject".to_string()),
        ("isInCameraView".to_string(), "isInCameraView".to_string()),
        ("occlusionQuery".to_string(), "setOcclusionQuery".to_string()),
        ("addLodLevel".to_string(), "addLODLevel".to_string()),
        ("removeLodLevel".to_string(), "removeLODLevel".to_string()),

        // === HAVOK PHYSICS V2 SYSTEM ===
        ("physics".to_string(), "enablePhysics".to_string()),
        ("disablePhysics".to_string(), "disablePhysics".to_string()),
        ("isPhysicsEnabled".to_string(), "isPhysicsEnabled".to_string()),
        ("gravity".to_string(), "setGravity".to_string()),
        ("getGravity".to_string(), "getGravity".to_string()),
        ("gravity".to_string(), "getGravity".to_string()), // Short alias for getGravity
        ("physicsAggregate".to_string(), "setPhysicsImpostor".to_string()), // V2 API using aggregate
        ("removePhysicsAggregate".to_string(), "removePhysicsImpostor".to_string()), // V2 API 
        ("hasPhysicsAggregate".to_string(), "hasPhysicsImpostor".to_string()), // V2 API
        ("updatePhysics".to_string(), "updatePhysics".to_string()), // V2 API for updating properties
        ("impulse".to_string(), "applyImpulse".to_string()),
        ("force".to_string(), "applyForce".to_string()),
        ("linearVelocity".to_string(), "setLinearVelocity".to_string()),
        ("getLinearVelocity".to_string(), "getLinearVelocity".to_string()),
        ("linearVelocity".to_string(), "getLinearVelocity".to_string()), // Short alias for getLinearVelocity
        ("angularVelocity".to_string(), "setAngularVelocity".to_string()),
        ("getAngularVelocity".to_string(), "getAngularVelocity".to_string()),
        ("angularVelocity".to_string(), "getAngularVelocity".to_string()), // Short alias for getAngularVelocity
        ("mass".to_string(), "getMass".to_string()),
        ("setMass".to_string(), "setMass".to_string()),
        ("friction".to_string(), "getFriction".to_string()),
        ("setFriction".to_string(), "setFriction".to_string()),
        ("restitution".to_string(), "getRestitution".to_string()),
        ("setRestitution".to_string(), "setRestitution".to_string()),
        ("physicsJoint".to_string(), "createPhysicsJoint".to_string()),
        ("removePhysicsJoint".to_string(), "removePhysicsJoint".to_string()),
        ("onCollisionEnter".to_string(), "onCollisionEnter".to_string()),
        ("onCollisionExit".to_string(), "onCollisionExit".to_string()),
        ("physicsRaycast".to_string(), "physicsRaycast".to_string()),
        ("characterController".to_string(), "createCharacterController".to_string()),
        ("moveCharacter".to_string(), "moveCharacter".to_string()),
        ("jumpCharacter".to_string(), "jumpCharacter".to_string()),
        ("ragdoll".to_string(), "enableRagdoll".to_string()),
        ("disableRagdoll".to_string(), "disableRagdoll".to_string()),
        ("softBody".to_string(), "enableSoftBody".to_string()),
        ("softBodyProperties".to_string(), "setSoftBodyProperties".to_string()),
        ("physicsMaterial".to_string(), "createPhysicsMaterial".to_string()),
        ("setPhysicsMaterial".to_string(), "setPhysicsMaterial".to_string()),
        ("pausePhysics".to_string(), "pausePhysics".to_string()),
        ("resumePhysics".to_string(), "resumePhysics".to_string()),
        ("physicsTimeStep".to_string(), "setPhysicsTimeStep".to_string()),
        ("physicsDebug".to_string(), "enablePhysicsDebug".to_string()),
        ("disablePhysicsDebug".to_string(), "disablePhysicsDebug".to_string()),
        ("disposePhysics".to_string(), "disposePhysics".to_string()),

        // === INPUT SYSTEM ===
        ("isKeyPressed".to_string(), "isKeyPressed".to_string()),
        ("isKeyDown".to_string(), "isKeyDown".to_string()),
        ("isAnyKeyPressed".to_string(), "isAnyKeyPressed".to_string()),
        ("getPressedKeys".to_string(), "getPressedKeys".to_string()),
        ("pressedKeys".to_string(), "getPressedKeys".to_string()), // Short alias for getPressedKeys
        ("isKeyComboPressed".to_string(), "isKeyComboPressed".to_string()),
        ("isCtrlPressed".to_string(), "isCtrlPressed".to_string()),
        ("isShiftPressed".to_string(), "isShiftPressed".to_string()),
        ("isAltPressed".to_string(), "isAltPressed".to_string()),
        ("isMousePressed".to_string(), "isMouseButtonPressed".to_string()),
        ("isLeftMouse".to_string(), "isLeftMouseButtonPressed".to_string()),
        ("isRightMouse".to_string(), "isRightMouseButtonPressed".to_string()),
        ("isMiddleMouse".to_string(), "isMiddleMouseButtonPressed".to_string()),
        ("mousePosition".to_string(), "getMousePosition".to_string()),
        ("mouseX".to_string(), "getMouseX".to_string()),
        ("mouseY".to_string(), "getMouseY".to_string()),
        ("mouseNormalized".to_string(), "getMouseNormalized".to_string()),
        ("touchCount".to_string(), "getTouchCount".to_string()),
        ("getTouches".to_string(), "getTouches".to_string()),
        ("touches".to_string(), "getTouches".to_string()), // Short alias for getTouches
        ("getTouch".to_string(), "getTouch".to_string()),
        ("touch".to_string(), "getTouch".to_string()), // Short alias for getTouch
        ("isTouching".to_string(), "isTouching".to_string()),
        ("pinchDistance".to_string(), "getPinchDistance".to_string()),
        ("touchCenter".to_string(), "getTouchCenter".to_string()),
        ("getGamepads".to_string(), "getGamepads".to_string()),
        ("gamepads".to_string(), "getGamepads".to_string()), // Short alias for getGamepads
        ("getGamepad".to_string(), "getGamepad".to_string()),
        ("gamepad".to_string(), "getGamepad".to_string()), // Short alias for getGamepad
        ("isGamepadConnected".to_string(), "isGamepadConnected".to_string()),
        ("button".to_string(), "isGamepadButtonPressed".to_string()),
        ("buttonValue".to_string(), "getGamepadButtonValue".to_string()),
        ("leftStick".to_string(), "getLeftStick".to_string()),
        ("rightStick".to_string(), "getRightStick".to_string()),
        ("leftX".to_string(), "getLeftStickX".to_string()),
        ("leftY".to_string(), "getLeftStickY".to_string()),
        ("rightX".to_string(), "getRightStickX".to_string()),
        ("rightY".to_string(), "getRightStickY".to_string()),
        ("leftTrigger".to_string(), "getLeftTrigger".to_string()),
        ("rightTrigger".to_string(), "getRightTrigger".to_string()),
        ("trigger".to_string(), "getGamepadTrigger".to_string()),
        ("isButtonA".to_string(), "isGamepadButtonA".to_string()),
        ("isButtonB".to_string(), "isGamepadButtonB".to_string()),
        ("isButtonX".to_string(), "isGamepadButtonX".to_string()),
        ("isButtonY".to_string(), "isGamepadButtonY".to_string()),
        ("deadzone".to_string(), "applyDeadzone".to_string()),
        ("leftStickDeadzone".to_string(), "getLeftStickWithDeadzone".to_string()),
        ("rightStickDeadzone".to_string(), "getRightStickWithDeadzone".to_string()),
        ("onKeyDown".to_string(), "onKeyDown".to_string()),
        ("onKeyUp".to_string(), "onKeyUp".to_string()),
        ("onMouseDown".to_string(), "onMouseDown".to_string()),
        ("onMouseUp".to_string(), "onMouseUp".to_string()),
        ("pointerLock".to_string(), "requestPointerLock".to_string()),
        ("exitPointerLock".to_string(), "exitPointerLock".to_string()),
        ("isPointerLocked".to_string(), "isPointerLocked".to_string()),
        ("virtualJoystick".to_string(), "createVirtualJoystick".to_string()),
        ("vibrate".to_string(), "vibrateGamepad".to_string()),
        ("inputSnapshot".to_string(), "getInputSnapshot".to_string()),

        // === SCENE MANAGEMENT ===
        ("sceneInfo".to_string(), "getSceneInfo".to_string()),
        ("performanceMonitor".to_string(), "enablePerformanceMonitor".to_string()),
        ("disablePerformanceMonitor".to_string(), "disablePerformanceMonitor".to_string()),
        ("performanceData".to_string(), "getPerformanceData".to_string()),

        // === ALL MATERIAL TYPES ===
        ("standardMaterial".to_string(), "createStandardMaterial".to_string()),
        ("pbrMaterial".to_string(), "createPBRMaterial".to_string()),
        ("pbrMetallicRoughnessMaterial".to_string(), "createPBRMetallicRoughnessMaterial".to_string()),
        ("pbrSpecularGlossinessMaterial".to_string(), "createPBRSpecularGlossinessMaterial".to_string()),
        ("unlitMaterial".to_string(), "createUnlitMaterial".to_string()),
        ("backgroundMaterial".to_string(), "createBackgroundMaterial".to_string()),
        ("nodeMaterial".to_string(), "createNodeMaterial".to_string()),
        ("shaderMaterial".to_string(), "createShaderMaterial".to_string()),
        ("multiMaterial".to_string(), "createMultiMaterial".to_string()),
        ("cellMaterial".to_string(), "createCellMaterial".to_string()),
        ("customMaterial".to_string(), "createCustomMaterial".to_string()),
        ("pbrCustomMaterial".to_string(), "createPBRCustomMaterial".to_string()),
        ("simpleMaterial".to_string(), "createSimpleMaterial".to_string()),
        ("shadowOnlyMaterial".to_string(), "createShadowOnlyMaterial".to_string()),
        ("skyMaterial".to_string(), "createSkyMaterial".to_string()),
        ("waterMaterial".to_string(), "createWaterMaterial".to_string()),
        ("terrainMaterial".to_string(), "createTerrainMaterial".to_string()),
        ("gridMaterial".to_string(), "createGridMaterial".to_string()),
        ("triplanarMaterial".to_string(), "createTriPlanarMaterial".to_string()),
        ("mixMaterial".to_string(), "createMixMaterial".to_string()),
        ("lavaMaterial".to_string(), "createLavaMaterial".to_string()),
        ("fireMaterial".to_string(), "createFireMaterial".to_string()),
        ("furMaterial".to_string(), "createFurMaterial".to_string()),
        ("gradientMaterial".to_string(), "createGradientMaterial".to_string()),

        // === ALL TEXTURE TYPES ===
        ("texture".to_string(), "createTexture".to_string()),
        ("cubeTexture".to_string(), "createCubeTexture".to_string()),
        ("hdrCubeTexture".to_string(), "createHDRCubeTexture".to_string()),
        ("videoTexture".to_string(), "createVideoTexture".to_string()),
        ("mirrorTexture".to_string(), "createMirrorTexture".to_string()),
        ("refractionTexture".to_string(), "createRefractionTexture".to_string()),
        ("depthTexture".to_string(), "createDepthTexture".to_string()),

        // === PROCEDURAL TEXTURES ===
        ("proceduralTexture".to_string(), "createProceduralTexture".to_string()),
        ("noiseTexture".to_string(), "createNoiseTexture".to_string()),
        ("woodTexture".to_string(), "createWoodTexture".to_string()),
        ("marbleTexture".to_string(), "createMarbleTexture".to_string()),
        ("fireTexture".to_string(), "createFireTexture".to_string()),
        ("cloudTexture".to_string(), "createCloudTexture".to_string()),
        ("grassTexture".to_string(), "createGrassTexture".to_string()),
        ("roadTexture".to_string(), "createRoadTexture".to_string()),
        ("brickTexture".to_string(), "createBrickTexture".to_string()),
        ("perlinNoiseTexture".to_string(), "createPerlinNoiseTexture".to_string()),
        ("normalMapTexture".to_string(), "createNormalMapTexture".to_string()),

        // === ALL MESH BUILDERS ===
        ("box".to_string(), "createBox".to_string()),
        ("sphere".to_string(), "createSphere".to_string()),
        ("cylinder".to_string(), "createCylinder".to_string()),
        ("plane".to_string(), "createPlane".to_string()),
        ("ground".to_string(), "createGround".to_string()),
        ("torus".to_string(), "createTorus".to_string()),
        ("tube".to_string(), "createTube".to_string()),
        ("ribbon".to_string(), "createRibbon".to_string()),
        ("lathe".to_string(), "createLathe".to_string()),
        ("extrusion".to_string(), "createExtrusion".to_string()),
        ("polygon".to_string(), "createPolygon".to_string()),
        ("icosphere".to_string(), "createIcosphere".to_string()),
        ("capsule".to_string(), "createCapsule".to_string()),
        ("text".to_string(), "createText".to_string()),
        ("decal".to_string(), "createDecal".to_string()),
        ("lineSystem".to_string(), "createLineSystem".to_string()),
        ("dashedLines".to_string(), "createDashedLines".to_string()),
        ("trail".to_string(), "createTrail".to_string()),

        // === ALL CAMERA TYPES ===
        ("isCamera".to_string(), "isCamera".to_string()),
        ("cameraFov".to_string(), "setCameraFOV".to_string()),
        ("getCameraFov".to_string(), "getCameraFOV".to_string()),
        ("cameraFov".to_string(), "getCameraFOV".to_string()), // Short alias for getCameraFov
        ("cameraType".to_string(), "setCameraType".to_string()),
        ("orbitCamera".to_string(), "orbitCamera".to_string()),
        ("detachCameraControls".to_string(), "detachCameraControls".to_string()),
        ("attachCameraControls".to_string(), "attachCameraControls".to_string()),
        ("cameraRadius".to_string(), "setCameraRadius".to_string()),
        ("getCameraRadius".to_string(), "getCameraRadius".to_string()),
        ("cameraRadius".to_string(), "getCameraRadius".to_string()), // Short alias for getCameraRadius
        ("arcRotateCamera".to_string(), "createArcRotateCamera".to_string()),
        ("freeCamera".to_string(), "createFreeCamera".to_string()),
        ("universalCamera".to_string(), "createUniversalCamera".to_string()),
        ("flyCamera".to_string(), "createFlyCamera".to_string()),
        ("followCamera".to_string(), "createFollowCamera".to_string()),
        ("deviceOrientationCamera".to_string(), "createDeviceOrientationCamera".to_string()),
        ("virtualJoysticksCamera".to_string(), "createVirtualJoysticksCamera".to_string()),
        ("webvrFreeCamera".to_string(), "createWebVRFreeCamera".to_string()),
        ("vrDeviceOrientationCamera".to_string(), "createVRDeviceOrientationCamera".to_string()),

        // === ALL LIGHT TYPES ===
        ("isLight".to_string(), "isLight".to_string()),
        ("lightIntensity".to_string(), "setLightIntensity".to_string()),
        ("getLightIntensity".to_string(), "getLightIntensity".to_string()),
        ("lightIntensity".to_string(), "getLightIntensity".to_string()), // Short alias for getLightIntensity
        ("lightColor".to_string(), "setLightColor".to_string()),
        ("getLightColor".to_string(), "getLightColor".to_string()),
        ("lightColor".to_string(), "getLightColor".to_string()), // Short alias for getLightColor
        ("lightRange".to_string(), "setLightRange".to_string()),
        ("getLightRange".to_string(), "getLightRange".to_string()),
        ("lightRange".to_string(), "getLightRange".to_string()), // Short alias for getLightRange
        ("ensureLight".to_string(), "ensureLight".to_string()),
        ("lightPosition".to_string(), "setLightPosition".to_string()),
        ("lightDirection".to_string(), "setLightDirection".to_string()),
        ("lightSpecular".to_string(), "setLightSpecular".to_string()),
        ("hemisphericGroundColor".to_string(), "setHemisphericGroundColor".to_string()),
        ("sceneExposure".to_string(), "setSceneExposure".to_string()),
        ("shadowEnabled".to_string(), "setShadowEnabled".to_string()),
        ("shadowDarkness".to_string(), "setShadowDarkness".to_string()),
        ("shadowBias".to_string(), "setShadowBias".to_string()),
        ("shadowQuality".to_string(), "setShadowQuality".to_string()),
        ("shadowSoftness".to_string(), "setShadowSoftness".to_string()),
        ("directionalLight".to_string(), "createDirectionalLight".to_string()),
        ("hemisphericLight".to_string(), "createHemisphericLight".to_string()),
        ("pointLight".to_string(), "createPointLight".to_string()),
        ("spotLight".to_string(), "createSpotLight".to_string()),

        // === SKYBOX API ===
        ("ensureSkybox".to_string(), "ensureSkybox".to_string()),
        ("skyboxColors".to_string(), "setSkyboxColors".to_string()),
        ("skyboxTexture".to_string(), "setSkyboxTexture".to_string()),
        ("skyboxSize".to_string(), "setSkyboxSize".to_string()),
        ("skyboxEnabled".to_string(), "setSkyboxEnabled".to_string()),
        ("skyboxInfinite".to_string(), "setSkyboxInfinite".to_string()),

        // === PARTICLE SYSTEMS ===
        ("particleSystem".to_string(), "createParticleSystem".to_string()),
        ("gpuParticleSystem".to_string(), "createGPUParticleSystem".to_string()),
        ("solidParticleSystem".to_string(), "createSolidParticleSystem".to_string()),
        ("pointsCloudSystem".to_string(), "createPointsCloudSystem".to_string()),
        ("startParticles".to_string(), "startParticles".to_string()),
        ("stopParticles".to_string(), "stopParticles".to_string()),
        ("particleEmissionRate".to_string(), "setParticleEmissionRate".to_string()),
        ("particleLifeTime".to_string(), "setParticleLifeTime".to_string()),
        ("particleSize".to_string(), "setParticleSize".to_string()),
        ("particleColor".to_string(), "setParticleColor".to_string()),
        ("particleVelocity".to_string(), "setParticleVelocity".to_string()),
        ("particleGravity".to_string(), "setParticleGravity".to_string()),
        ("particleTexture".to_string(), "setParticleTexture".to_string()),

        // === POST-PROCESSING PIPELINES ===
        ("defaultRenderingPipeline".to_string(), "createDefaultRenderingPipeline".to_string()),
        ("ssaoRenderingPipeline".to_string(), "createSSAORenderingPipeline".to_string()),
        ("ssao2RenderingPipeline".to_string(), "createSSAO2RenderingPipeline".to_string()),
        ("standardRenderingPipeline".to_string(), "createStandardRenderingPipeline".to_string()),
        ("lensRenderingPipeline".to_string(), "createLensRenderingPipeline".to_string()),
        ("addPostProcess".to_string(), "addPostProcess".to_string()),
        ("removePostProcess".to_string(), "removePostProcess".to_string()),
        ("blurPostProcess".to_string(), "createBlurPostProcess".to_string()),
        ("blackAndWhitePostProcess".to_string(), "createBlackAndWhitePostProcess".to_string()),
        ("convolutionPostProcess".to_string(), "createConvolutionPostProcess".to_string()),
        ("filterPostProcess".to_string(), "createFilterPostProcess".to_string()),
        ("fxaaPostProcess".to_string(), "createFxaaPostProcess".to_string()),
        ("highlightsPostProcess".to_string(), "createHighlightsPostProcess".to_string()),
        ("refractionPostProcess".to_string(), "createRefractionPostProcess".to_string()),
        ("volumetricLightPostProcess".to_string(), "createVolumetricLightPostProcess".to_string()),
        ("colorCorrectionPostProcess".to_string(), "createColorCorrectionPostProcess".to_string()),
        ("tonemapPostProcess".to_string(), "createTonemapPostProcess".to_string()),
        ("imageProcessingPostProcess".to_string(), "createImageProcessingPostProcess".to_string()),

        // === GUI 2D SYSTEM ===
        ("guiTexture".to_string(), "createGUITexture".to_string()),
        ("guiButton".to_string(), "createGUIButton".to_string()),
        ("guiTextBlock".to_string(), "createGUITextBlock".to_string()),
        ("guiStackPanel".to_string(), "createGUIStackPanel".to_string()),
        ("guiRectangle".to_string(), "createGUIRectangle".to_string()),
        ("guiEllipse".to_string(), "createGUIEllipse".to_string()),
        ("guiLine".to_string(), "createGUILine".to_string()),
        ("guiSlider".to_string(), "createGUISlider".to_string()),
        ("guiCheckbox".to_string(), "createGUICheckBox".to_string()),
        ("guiRadioButton".to_string(), "createGUIRadioButton".to_string()),
        ("guiInputText".to_string(), "createGUIInputText".to_string()),
        ("guiPassword".to_string(), "createGUIPassword".to_string()),
        ("guiScrollViewer".to_string(), "createGUIScrollViewer".to_string()),
        ("guiVirtualKeyboard".to_string(), "createGUIVirtualKeyboard".to_string()),
        ("guiImage".to_string(), "createGUIImage".to_string()),

        // === GUI 3D SYSTEM ===
        ("gui3dManager".to_string(), "createGUI3DManager".to_string()),
        ("cylinderPanel".to_string(), "createCylinderPanel".to_string()),
        ("planePanel".to_string(), "createPlanePanel".to_string()),
        ("spherePanel".to_string(), "createSpherePanel".to_string()),
        ("stackPanel3d".to_string(), "createStackPanel3D".to_string()),
        ("button3d".to_string(), "createButton3D".to_string()),
        ("holographicButton".to_string(), "createHolographicButton".to_string()),
        ("meshButton3d".to_string(), "createMeshButton3D".to_string()),

        // === XR/VR/AR SYSTEM ===
        ("webxrDefaultExperience".to_string(), "createWebXRDefaultExperience".to_string()),
        ("webxrExperienceHelper".to_string(), "createWebXRExperienceHelper".to_string()),
        ("webxr".to_string(), "enableWebXR".to_string()),
        ("disableWebxr".to_string(), "disableWebXR".to_string()),
        ("isWebxrAvailable".to_string(), "isWebXRAvailable".to_string()),
        ("isWebxrSessionActive".to_string(), "isWebXRSessionActive".to_string()),
        ("webxrControllers".to_string(), "getWebXRControllers".to_string()),
        ("webxrInputSources".to_string(), "getWebXRInputSources".to_string()),
        ("teleportInXr".to_string(), "teleportInXR".to_string()),
        ("handTracking".to_string(), "enableHandTracking".to_string()),
        ("disableHandTracking".to_string(), "disableHandTracking".to_string()),

        // === BEHAVIOR SYSTEM ===
        ("autoRotationBehavior".to_string(), "addAutoRotationBehavior".to_string()),
        ("bouncingBehavior".to_string(), "addBouncingBehavior".to_string()),
        ("framingBehavior".to_string(), "addFramingBehavior".to_string()),
        ("attachToBoxBehavior".to_string(), "addAttachToBoxBehavior".to_string()),
        ("fadeInOutBehavior".to_string(), "addFadeInOutBehavior".to_string()),
        ("multiPointerScaleBehavior".to_string(), "addMultiPointerScaleBehavior".to_string()),
        ("pointerDragBehavior".to_string(), "addPointerDragBehavior".to_string()),
        ("sixDofDragBehavior".to_string(), "addSixDofDragBehavior".to_string()),
        ("removeBehavior".to_string(), "removeBehavior".to_string()),
        ("getBehaviors".to_string(), "getBehaviors".to_string()),

        // === GIZMO SYSTEM ===
        ("gizmoManager".to_string(), "createGizmoManager".to_string()),
        ("positionGizmo".to_string(), "createPositionGizmo".to_string()),
        ("rotationGizmo".to_string(), "createRotationGizmo".to_string()),
        ("scaleGizmo".to_string(), "createScaleGizmo".to_string()),
        ("boundingBoxGizmo".to_string(), "showBoundingBoxGizmo".to_string()),
        ("gizmos".to_string(), "enableGizmos".to_string()),
        ("disableGizmos".to_string(), "disableGizmos".to_string()),

        // === LAYER SYSTEM ===
        ("layer".to_string(), "createLayer".to_string()),
        ("highlightLayer".to_string(), "createHighlightLayer".to_string()),
        ("glowLayer".to_string(), "createGlowLayer".to_string()),
        ("effectLayer".to_string(), "createEffectLayer".to_string()),
        ("addToHighlightLayer".to_string(), "addToHighlightLayer".to_string()),
        ("removeFromHighlightLayer".to_string(), "removeFromHighlightLayer".to_string()),
        ("addToGlowLayer".to_string(), "addToGlowLayer".to_string()),
        ("removeFromGlowLayer".to_string(), "removeFromGlowLayer".to_string()),

        // === SPRITE SYSTEM ===
        ("sprite".to_string(), "createSprite".to_string()),
        ("spriteManager".to_string(), "createSpriteManager".to_string()),
        ("spriteMap".to_string(), "createSpriteMap".to_string()),
        ("spriteTexture".to_string(), "setSpriteTexture".to_string()),
        ("spriteFrame".to_string(), "setSpriteFrame".to_string()),
        ("animateSprite".to_string(), "animateSprite".to_string()),
        ("disposeSprite".to_string(), "disposeSprite".to_string()),

        // === MORPH TARGET SYSTEM ===
        ("morphTarget".to_string(), "createMorphTarget".to_string()),
        ("morphTargetManager".to_string(), "createMorphTargetManager".to_string()),
        ("addMorphTarget".to_string(), "addMorphTarget".to_string()),
        ("removeMorphTarget".to_string(), "removeMorphTarget".to_string()),
        ("morphTargetInfluence".to_string(), "setMorphTargetInfluence".to_string()),
        ("getMorphTargetInfluence".to_string(), "getMorphTargetInfluence".to_string()),
        ("morphTargetInfluence".to_string(), "getMorphTargetInfluence".to_string()), // Short alias for getMorphTargetInfluence

        // === NAVIGATION & CROWD ===
        ("navigationMesh".to_string(), "createNavigationMesh".to_string()),
        ("findPath".to_string(), "findPath".to_string()),
        ("crowd".to_string(), "createCrowd".to_string()),
        ("addAgentToCrowd".to_string(), "addAgentToCrowd".to_string()),
        ("removeAgentFromCrowd".to_string(), "removeAgentFromCrowd".to_string()),
        ("agentDestination".to_string(), "setAgentDestination".to_string()),
        ("getAgentPosition".to_string(), "getAgentPosition".to_string()),
        ("agentPosition".to_string(), "getAgentPosition".to_string()), // Short alias for getAgentPosition
        ("getAgentVelocity".to_string(), "getAgentVelocity".to_string()),
        ("agentVelocity".to_string(), "getAgentVelocity".to_string()), // Short alias for getAgentVelocity

        // === BAKED VERTEX ANIMATION ===
        ("bakedVertexAnimation".to_string(), "createBakedVertexAnimation".to_string()),
        ("bakeVertexAnimation".to_string(), "bakeVertexAnimation".to_string()),
        ("playBakedAnimation".to_string(), "playBakedAnimation".to_string()),

        // === COMPUTE SHADERS ===
        ("computeShader".to_string(), "createComputeShader".to_string()),
        ("computeEffect".to_string(), "createComputeEffect".to_string()),
        ("dispatchCompute".to_string(), "dispatchCompute".to_string()),
        ("computeUniform".to_string(), "setComputeUniform".to_string()),
        ("getComputeBuffer".to_string(), "getComputeBuffer".to_string()),

        // === FLOW GRAPH SYSTEM ===
        ("flowGraph".to_string(), "createFlowGraph".to_string()),
        ("flowGraphBlock".to_string(), "addFlowGraphBlock".to_string()),
        ("connectFlowGraphNodes".to_string(), "connectFlowGraphNodes".to_string()),
        ("executeFlowGraph".to_string(), "executeFlowGraph".to_string()),

        // === FRAME GRAPH SYSTEM ===
        ("frameGraph".to_string(), "createFrameGraph".to_string()),
        ("frameGraphTask".to_string(), "addFrameGraphTask".to_string()),
        ("executeFrameGraph".to_string(), "executeFrameGraph".to_string()),

        // === DEBUG & VISUALIZATION ===
        ("axesViewer".to_string(), "createAxesViewer".to_string()),
        ("boneAxesViewer".to_string(), "createBoneAxesViewer".to_string()),
        ("skeletonViewer".to_string(), "createSkeletonViewer".to_string()),
        ("physicsViewer".to_string(), "createPhysicsViewer".to_string()),
        ("rayHelper".to_string(), "createRayHelper".to_string()),
        ("debugLayer".to_string(), "enableDebugLayer".to_string()),
        ("disableDebugLayer".to_string(), "disableDebugLayer".to_string()),
        ("showWorldAxes".to_string(), "showWorldAxes".to_string()),
        ("hideWorldAxes".to_string(), "hideWorldAxes".to_string()),

        // === ASSET LOADING ===
        ("loadMesh".to_string(), "loadMesh".to_string()),
        ("loadGltf".to_string(), "loadGLTF".to_string()),
        ("loadAssetContainer".to_string(), "loadAssetContainer".to_string()),
        ("importMesh".to_string(), "importMesh".to_string()),
        ("appendScene".to_string(), "appendScene".to_string()),
        ("assetsManager".to_string(), "createAssetsManager".to_string()),
        ("meshTask".to_string(), "addMeshTask".to_string()),
        ("textureTask".to_string(), "addTextureTask".to_string()),
        ("loadAllAssets".to_string(), "loadAllAssets".to_string()),
        ("mergeModelWithSkeleton".to_string(), "mergeModelWithSkeleton".to_string()),
        ("loadAndMergeAssets".to_string(), "loadAndMergeAssets".to_string()),
        ("getLoadedAsset".to_string(), "getLoadedAsset".to_string()),
        ("getLoadedMesh".to_string(), "getLoadedMesh".to_string()),
        ("getLoadedAnimations".to_string(), "getLoadedAnimations".to_string()),
        ("getLoadedSkeleton".to_string(), "getLoadedSkeleton".to_string()),

        // === SERIALIZATION ===
        ("serializeScene".to_string(), "serializeScene".to_string()),
        ("exportGltf".to_string(), "exportGLTF".to_string()),
        ("exportObj".to_string(), "exportOBJ".to_string()),
        ("exportStl".to_string(), "exportSTL".to_string()),
        ("exportUsdz".to_string(), "exportUSDZ".to_string()),
        ("exportSplat".to_string(), "exportSplat".to_string()),

        // === AUDIO V2 SYSTEM ===
        ("playSound".to_string(), "playSound".to_string()),
        ("stopSound".to_string(), "stopSound".to_string()),
        ("soundVolume".to_string(), "setSoundVolume".to_string()),
        ("sound".to_string(), "createSound".to_string()),
        ("soundTrack".to_string(), "createSoundTrack".to_string()),
        ("spatialSound".to_string(), "createSpatialSound".to_string()),
        ("soundPosition".to_string(), "setSoundPosition".to_string()),
        ("soundMaxDistance".to_string(), "setSoundMaxDistance".to_string()),
        ("soundRolloffFactor".to_string(), "setSoundRolloffFactor".to_string()),
        ("audioAnalyser".to_string(), "createAudioAnalyser".to_string()),
        ("audioFrequencyData".to_string(), "getAudioFrequencyData".to_string()),
        ("audioTimeData".to_string(), "getAudioTimeData".to_string()),

        // === ALL EASING FUNCTIONS ===
        ("circleEase".to_string(), "createCircleEase".to_string()),
        ("backEase".to_string(), "createBackEase".to_string()),
        ("bounceEase".to_string(), "createBounceEase".to_string()),
        ("cubicEase".to_string(), "createCubicEase".to_string()),
        ("elasticEase".to_string(), "createElasticEase".to_string()),
        ("exponentialEase".to_string(), "createExponentialEase".to_string()),
        ("powerEase".to_string(), "createPowerEase".to_string()),
        ("quadraticEase".to_string(), "createQuadraticEase".to_string()),
        ("quarticEase".to_string(), "createQuarticEase".to_string()),
        ("quinticEase".to_string(), "createQuinticEase".to_string()),
        ("sineEase".to_string(), "createSineEase".to_string()),
        ("bezierCurveEase".to_string(), "createBezierCurveEase".to_string()),

        // === CSG OPERATIONS ===
        ("csg".to_string(), "createCSG".to_string()),
        ("csgUnion".to_string(), "csgUnion".to_string()),
        ("csgSubtract".to_string(), "csgSubtract".to_string()),
        ("csgIntersect".to_string(), "csgIntersect".to_string()),
        ("csgToMesh".to_string(), "csgToMesh".to_string()),

        // === INSTANCING ===
        ("instances".to_string(), "createInstances".to_string()),
        ("thinInstances".to_string(), "createThinInstances".to_string()),
        ("updateInstanceData".to_string(), "updateInstanceData".to_string()),
        ("disposeInstances".to_string(), "disposeInstances".to_string()),
        ("getInstanceCount".to_string(), "getInstanceCount".to_string()),
        ("instanceCount".to_string(), "getInstanceCount".to_string()), // Short alias for getInstanceCount

        // === RENDERING OPTIMIZATION ===
        ("freezeWorldMatrix".to_string(), "freezeWorldMatrix".to_string()),
        ("unfreezeWorldMatrix".to_string(), "unfreezeWorldMatrix".to_string()),
        ("renderingGroup".to_string(), "setRenderingGroup".to_string()),
        ("getRenderingGroup".to_string(), "getRenderingGroup".to_string()),
        ("renderingGroup".to_string(), "getRenderingGroup".to_string()), // Short alias for getRenderingGroup
        ("layerMask".to_string(), "setLayerMask".to_string()),
        ("getLayerMask".to_string(), "getLayerMask".to_string()),
        ("layerMask".to_string(), "getLayerMask".to_string()), // Short alias for getLayerMask
        ("edges".to_string(), "enableEdges".to_string()),
        ("disableEdges".to_string(), "disableEdges".to_string()),
        ("outline".to_string(), "enableOutline".to_string()),
        ("disableOutline".to_string(), "disableOutline".to_string()),
        ("outlineColor".to_string(), "setOutlineColor".to_string()),
        ("outlineWidth".to_string(), "setOutlineWidth".to_string()),

        // === ENVIRONMENT & HELPERS ===
        ("environmentHelper".to_string(), "createEnvironmentHelper".to_string()),
        ("photoDome".to_string(), "createPhotoDome".to_string()),
        ("videoDome".to_string(), "createVideoDome".to_string()),
        ("textureDome".to_string(), "createTextureDome".to_string()),

        // === ADVANCED RENDERING ===
        ("depthRenderer".to_string(), "enableDepthRenderer".to_string()),
        ("geometryBufferRenderer".to_string(), "enableGeometryBufferRenderer".to_string()),
        ("outlineRenderer".to_string(), "enableOutlineRenderer".to_string()),
        ("edgesRenderer".to_string(), "enableEdgesRenderer".to_string()),
        ("boundingBoxRenderer".to_string(), "enableBoundingBoxRenderer".to_string()),
        ("utilityLayerRenderer".to_string(), "createUtilityLayerRenderer".to_string()),

        // === DYNAMIC PROPERTIES ===
        ("dynamicProperty".to_string(), "addDynamicProperty".to_string()),
        ("updatePropertyOptions".to_string(), "updatePropertyOptions".to_string()),
        ("removeDynamicProperty".to_string(), "removeDynamicProperty".to_string()),
        ("getPropertyValue".to_string(), "getPropertyValue".to_string()),
        ("propertyValue".to_string(), "getPropertyValue".to_string()), // Short alias for getPropertyValue
        ("propertyValue".to_string(), "setPropertyValue".to_string()),
    ]
}