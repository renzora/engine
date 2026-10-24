# Facial Animation

A face rig gives a head bones for the jaw, the lips, the mouth corners, both eyes, all four eyelids and both brows. In the [Animation workspace](/docs/r1-alpha8/editor/animation#the-animation-workspace) you build it in **Rig** mode and animate it in **Animate**: expressions, blinking and talking.

<!-- screenshot: face_animate.png - Animate mode on a head: the Face tab's expression sliders on the left, the face on the stage mid-smile, the Performance tab's lip sync on the right -->

## Rigging a face

Most heads arrive with no skeleton and no blend shapes: a scan, a sculpt, a bust from an asset store. Put the head on the stage; a model with eye meshes and no skeleton opens in Rig mode with the **Face** template chosen.

1. Check **Meshes**: one row per kind of mesh the model was found to have, judged from the meshes' names, with how many of each. Hover a row for the names of its meshes.
   - **Skin** is weighted to the face bones.
   - **Eyes** (any mesh named `eye`) turn with their eye bone; they are also how the rig finds the eyes and sizes everything around them.
   - **Lashes** follow the lids.
   - **Mouth interior** (teeth, tongue) splits at the lips: the lower half goes with the jaw.
   - **Rigid** parts (hair) stay with the head.
2. Click **Place Points**, then click the face at each point: a mouth corner (either side, the other is mirrored), the middle of the upper lip, the middle of the lower lip, and the chin. Front view (the Front button) makes this easiest.

   Put the corner where the lips meet. Around the mouth, each patch of skin follows whichever lip it is nearer to across the surface of the face, so the inside of each lip, folded into the mouth, moves with its own lip and the mouth opens rather than stretching. Farther out, across the cheeks and chin, the corner's height is where the jaw's skin begins. The lip points only need to be somewhere on their lip; a click right on the line between the lips can fall through the gap, so click a little into the lip.

   This needs lips that are separate surfaces, which is how a head made for animation is built. A head whose lips are modelled as one sealed surface has nothing to open: its jaw can only stretch the face.
3. Click **Build Face Rig**.

The model's file is rewritten with the rig in it, and every copy in your scenes updates. The original is kept beside it as `<name>.unrigged.glb`; **Remove Rig** puts it back, and rigging again starts from that original, so a face can be re-rigged with better points. To fine-tune how the face follows its bones, paint it in [Skin mode](/docs/r1-alpha8/editor/rigging#skin-how-the-mesh-follows-the-bones).

## Expressions

In **Animate**, the left panel's **Face** tab has a slider per expression: Jaw Open, Blink Left and Right, Smile, Frown, Pucker, Mouth Wide, Lips Part, Brows Up and Down, and Look Left/Right and Up/Down. They add together, so Smile and Jaw Open at once is an open smile. Blink closes each eye fully whatever its shape: when the rig is built, it measures how far each eye is open and how far its lids must travel to meet. A head rigged before this was added blinks by a stock amount; rig it again to measure its eyes. The face bones can also be clicked and turned on the stage like any bone.

Drag them to pose the face, then **Key Face** (or `K`) keys the face bones at the playhead. As with any pose, moving the playhead drops an unkeyed face and puts the sliders back to zero. **Reset Face** puts it back to neutral.

Keying needs a clip: if the head has none, create one from the timeline.

## Performance: talking and blinking

The right panel's **Performance** tab generates keys into the open clip. Ctrl+Z takes them back.

### Lip sync

**Choose Recording...** picks a voice recording (`.wav`, `.ogg`, `.mp3`, `.flac`); **Generate Lip Sync** keys the jaw and lips from its loudness, frame by frame: the mouth opens with the voice, quickly, and closes a little slower between syllables. The clip is lengthened to fit the recording if it is shorter.

- **Strength** is how far the mouth opens at full volume.
- **Smoothing** leaves out small changes in loudness: higher keeps fewer keys and a calmer mouth.

It follows how loud the voice is, not which sounds it makes, so it reads as talking rather than as matched mouth shapes. Play the recording alongside the clip to hear it.

### Blinks

**Generate Blinks** keys natural blinks across the whole clip: a quick close, a short hold and a slower open. **Shortest gap** and **Longest gap** set how often: the time between blinks is drawn from between the two. Generating again with the same clip length gives the same blinks.

## Blend shapes

A head that arrived with blend shapes (morph targets) gets a slider for each under the expressions, showing the shape live on the stage.
