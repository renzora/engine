# Screen Transition

Wipes and fades between scenes. Drive `progress` from a script or an animation: `0.0` is fully covered, `1.0` fully revealed.

It defaults to `1.0` on purpose. A transition that defaulted to `0.0` would blank the screen the moment it was added.

**Put it last on the stack.** A transition is the last thing between the finished frame and the player, so an effect running after it would filter the wipe itself and a fade to black would come out grey. It is also the one effect here that uses the framework's snapshot of the previous composited frame, which is what it wipes away from.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `screen_transition.wesl` from the Assets panel onto it.

**Settings:** Progress, Mode, Direction, Smoothness
