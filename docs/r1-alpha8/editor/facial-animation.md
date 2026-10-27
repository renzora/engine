# Facial Animation

A face rig gives a head bones for the jaw, the lips, the mouth corners, both eyes, all four eyelids and both brows. In the [Animation workspace](/docs/r1-alpha8/editor/animation#the-animation-workspace) you build it in **Rig** mode and animate it in **Animate**: expressions, blinking and talking.

<!-- screenshot: face_animate.png - Animate mode on a head: the Controls tab's face preset sliders on the left, the face on the stage mid-smile, the Performance tab's lip sync on the right -->

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

   A head with no eye meshes, such as a photo scan whose eyes are painted on the skin, asks for a fifth point: the middle of either eye (the other is mirrored). The jaw, lips, mouth corners and brows work as usual; Blink and the Look sliders have no effect, since there is no eye under the lids to cover or turn.
3. Click **Build Face Rig**.

The model's file is rewritten with the rig in it, and every copy in your scenes updates. The original is kept beside it as `<name>.unrigged.rvg` (`.unrigged.glb` for a model stored as `.glb`); **Remove Rig** puts it back, and rigging again starts from that original, so a face can be re-rigged with better points. To fine-tune how the face follows its bones, paint it in [Skin mode](/docs/r1-alpha8/editor/rigging#skin-how-the-mesh-follows-the-bones).

## Expressions

A face is animated with [controls](/docs/r1-alpha8/editor/rigging#controls): sliders, each a named pose of bones and blend shapes. In **Animate**, the left panel's **Controls** tab lists the model's controls. A head with a face rig and no controls yet offers **Use Face Preset**, which adds the face's built-in expressions as controls: Jaw Open, Blink Left and Right, Smile, Frown, Pucker, Mouth Wide, Lips Part, Brows Up and Down, and Look Left / Right and Up / Down. They are ordinary controls from then on: rename them, re-capture them with a better pose, change their range, delete the ones you do not use, and add your own (a sneer, a cheek puff) beside them.

Controls add together, so Smile and Jaw Open at once is an open smile. Blink closes each eye fully whatever its shape: when the rig is built, it measures how far each eye is open and how far its lids must travel to meet, and, where the eyeball bulges past the lids (most stylised eyes), how far forward the lids must come to close over it rather than behind it. The preset's blinks read those measurements from the head every time they move, so re-rigging a head with better points fixes its blinks without making the controls again. A head rigged before this was added blinks by a stock amount; rig it again to measure its eyes. The face bones can also be clicked and turned on the stage like any bone.

Drag the sliders to pose the face, then **Key Controls** (or `K` over the stage) keys the bones and blend shapes they move at the playhead. As with any pose, moving the playhead drops an unkeyed face and puts the sliders back to zero. **Reset Controls** puts it back to neutral.

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

A head that arrived with blend shapes (morph targets) gets a slider for each under its controls, showing the shape live on the stage. Set some, then **Save as Control** to make a slider that dials that mix in.

A head with no blend shapes, or without the ones you need, can have them made in the editor: switch the stage to **Shapes**, press **New Shape**, and sculpt it (a smile, a blink, the mouth shapes for each sound) with the sculpt brushes, mirrored across the face. Imported shapes can be resculpted, renamed or deleted there too. **Save Shapes** writes them into the model file, and they appear here as sliders straight away, ready to key. See [Sculpting blend shapes](/docs/r1-alpha8/editor/rigging#sculpting-blend-shapes).

Blend shapes are keyed like bones:

- Drag a slider to shape the face. The weight holds over the clip until the playhead moves, exactly like an unkeyed bone pose.
- **Key Blend Shapes** keys every slider you moved at the playhead. With none moved it keys every shape at its current weight, which is how to key the face as it is (a neutral pose before an expression, say).
- **Key Controls** and `K` (over the stage, the viewport or the timeline) key the sliders you moved too, along with the bones.
- With record armed, a slider is keyed as you drag it.

Each shape you key gets its own track in the timeline, one row per shape under the property tracks, marked with a face glyph and named after the shape (`jawOpen`); hover the row for the mesh it belongs to. Its keys select, drag, copy, paste and delete like bone keys, take the same interpolation modes (Auto, Linear, Constant, Flat, Free, Broken), and show as a single curve in the Curves view. A new key is Auto, so shapes ease into each other. Every edit can be undone with `Ctrl+Z` and plays straight away.

Blend shape tracks play through the animator at runtime like the skeleton does, so they crossfade between clips, sit on layers (a facial layer over a body clip) and blend in blend trees. A clip binds a shape by the mesh's name and the shape's number, so it plays on any model whose face mesh has the same name and shape order. A shape below the highest one a clip keys, and not keyed itself, plays at zero while the clip plays.

Blend shape animation in an imported model comes in as these tracks; see [Importing animations](/docs/r1-alpha8/editor/animation#importing-animations).
