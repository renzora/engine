// @effect(name = "Rain", icon = "cloud-rain")
//
// Rain on the lens: droplets and runnels over the finished frame, as if the
// camera itself is wet. It is a screen effect, not weather in the world.
//
// The glass fogs, and the water running down it wipes the fog away: that
// contrast is what reads as a wet windscreen rather than marks on the picture.
// `fog` sets how much it hazes between the water; 0 leaves the glass clear.
//
// `drop_size` is a size, so turning it up makes fewer, larger beads.
// `speed_variation` spreads the fall rate between columns (0 runs them all at
// `speed`). `trail` is the length of the runnel each bead drags, as a multiple
// of its own size; 0 leaves bare beads.
//
// Refraction comes from the slope of the water rather than being drawn per
// drop, so beads and runnels bend light by the same rule. That costs three
// evaluations of the drop field per pixel plus nine taps for the fog blur,
// because the screen target has no mip chain to blur against.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct RainSettings {
    intensity: f32,        // @slider(0, 1, default = 0.3, step = 0.01)
    speed: f32,            // @slider(0.1, 5, default = 1, step = 0.1)
    drop_size: f32,        // @slider(1, 20, default = 8, step = 0.1)
    speed_variation: f32,  // @slider(0, 1, default = 0.5, step = 0.01)
    trail: f32,            // @slider(0, 1, default = 0.5, step = 0.01)
    fog: f32,              // @slider(0, 1, default = 0.6, step = 0.01)
    time: f32,
};
@group(0) @binding(2) var<uniform> settings: RainSettings;

fn hash2(p: vec2<f32>) -> vec2<f32> {
    let q = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
    return fract(sin(q) * 43758.5453);
}

/// How wet the glass is at a point, as (all water, runnels only).
///
/// The whole effect is built on this one field rather than on drops drawn
/// individually. Water bends light along the *slope* of its own surface, so
/// once wetness is a smooth height field the refraction falls out of its
/// gradient: beads and runnels bend light correctly without either being a
/// special case. Drawing each drop's distortion by hand, as this shader used
/// to, meant the trails had to fake theirs and never quite matched.
fn wetness(g: vec2<f32>, t: f32, cols: f32) -> vec2<f32> {
    let cell = floor(g);
    let local = fract(g);

    // A cell is one column wide and the whole screen tall, so a grid step down
    // is `cols` times longer than a step across. Everything round (beads, the
    // distances between them) has to be measured after undoing that, or the
    // drops come out as hairlines.
    let ky = cols;

    var water = 0.0;
    var runnel = 0.0;

    // Beads that simply sit there. Most water on glass does nothing: without a
    // still layer the screen holds only as many drops as there are columns,
    // which is far too few to read as rain.
    let fine = g * vec2(3.0, 3.0 * ky);
    let fine_cell = floor(fine);
    let fine_local = fract(fine) - 0.5;
    let fine_n = hash2(fine_cell + 0.5);

    // Its own clock, offset per cell so they are not all in step. The whole
    // number is which bead this is, the fraction is how far through its life.
    let phase = t * 0.09 + fine_n.x;
    let sit_index = floor(phase);
    let sit_cycle = fract(phase);

    // Re-rolled every time. Taken from the cell alone, a bead reappeared in
    // exactly the spot the last one dried from, so the still layer blinked on
    // and off in a lattice of unchanging positions.
    let sit_roll = hash2(fine_cell + vec2(sit_index * 31.0, sit_index * 57.0));
    // And not every slot fills every time, or the lattice is visible in the
    // timing instead.
    let sit_gap = hash2(fine_cell + vec2(sit_index * 91.0, 3.0)).x;
    if sit_gap < 0.55 {
        let sitting = smoothstep(0.0, 0.12, sit_cycle) * smoothstep(1.0, 0.55, sit_cycle);
        // Kept inside the cell: there is no neighbourhood search for this
        // layer, so a bead wandering past the edge would be clipped in half.
        let fine_d = length(fine_local - (sit_roll - 0.5) * 0.44);
        let still_radius = 0.12 + sit_roll.y * 0.10;
        let still = smoothstep(still_radius, still_radius * 0.3, fine_d) * sitting;
        water = max(water, still * 0.8);
    }

    // Check the 3x3 neighbourhood, so a bead straddling a cell edge is seen by
    // both sides of it.
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let neighbor = cell + vec2(f32(x), f32(y));

            // Fixed per cell: where this column is in its cycle, and how fast
            // it runs. Everything else is re-rolled per drop, below.
            let column = hash2(neighbor);

            // Spread the fall rate around `speed`. At 0 every column runs at
            // exactly `speed`, which is the setting's off position; at 1 the
            // slowest crawls while the fastest runs at nearly double.
            let spread = (column.y - 0.5) * 1.8 * settings.speed_variation;
            let fall = max(0.15, 1.0 + spread);

            // `v` grows downward, so a falling drop ADDS time. The whole number
            // is which drop this is, the fraction is how far down it has got;
            // splitting them is what lets each pass be a new drop rather than
            // the same one going round again.
            let path = column.x * 831.0 + t * fall;
            let drop_index = floor(path);
            let fallen = fract(path);

            let roll = hash2(neighbor + vec2(drop_index * 17.0, drop_index * 29.0));
            let gap = hash2(neighbor + vec2(drop_index * 53.0, 7.0)).x;

            // Half the slots stay empty, or every cell always holds a bead and
            // the grid is plain to see.
            if gap > 0.5 {
                continue;
            }

            // Where this sample sits inside the neighbour's own cell, so the
            // wake below can be placed against the GLASS rather than against
            // the bead.
            let q = local - vec2(f32(x), f32(y));
            let drop_pos = vec2(roll.x, fallen);
            // Vertical distances scaled back to screen units, so a bead is a
            // bead and not a smear the height of the window.
            let delta = vec2(q.x - drop_pos.x, (q.y - drop_pos.y) * ky);
            // Beads this size read as water on glass. Larger and each one shows
            // a different part of the scene, which looks like a lens rather
            // than a drop.
            let radius = 0.06 + roll.y * 0.10;

            // A runner now crosses the whole screen, so it can reach the
            // bottom and leave rather than dissolving on the way down. All this
            // does is soften the half-bead that would otherwise pop in and out
            // at the two edges.
            let life = min(1.0, min(fallen, 1.0 - fallen) * 60.0);

            // The bead. Smooth rather than a hard circle: a step here would
            // give the gradient below nothing to work with but an edge.
            let bead = smoothstep(radius, radius * 0.25, length(delta)) * life;
            water = max(water, bead);

            // ── The wake it has already left ───────────────────────────────
            // Everything here is fixed to the glass and fades where it lies.
            // Measured backwards from the bead instead, the whole trail rode
            // down with it as one rigid shape: a comet, not a slug's trail.
            //
            // How far this glass is behind the bead, which is how long ago the
            // bead was on it. Positive only for glass it has already crossed.
            let behind = (drop_pos.y - q.y) * ky;
            let persist = radius * (4.0 + roll.x * 7.0) * settings.trail;
            if behind > 0.0 && behind < persist {
                // Older further back, so the wake thins out with age rather
                // than ending in a point. `sqrt` holds it then rounds it off.
                let age = behind / persist;
                let dried = sqrt(max(0.0, 1.0 - age));

                // The thin film, down the line the bead actually took.
                let half_width = radius * 0.3 * dried;
                let across = abs(q.x - drop_pos.x);
                let streak = smoothstep(half_width, half_width * 0.15, across)
                    * dried * life;
                // Faint on its own. Most of what reads as a trail is the beads
                // left standing in it, below.
                water = max(water, streak * 0.45);
                runnel = max(runnel, streak);

                // A sliding bead leaves a broken chain of smaller ones clinging
                // where it passed. Their slots are cut from the cell's own
                // height, so each stays put on the glass while the bead runs on
                // and the wake lengthens behind it.
                let spacing = radius * 1.1;
                let k = floor(q.y * ky / spacing);
                let jitter = hash2(neighbor + vec2(drop_index * 7.0 + k, k * 13.0));
                // Held in screen units, so the chain keeps its spacing no
                // matter how tall the cell is.
                let sit = vec2(
                    drop_pos.x + (jitter.x - 0.5) * radius * 0.5,
                    (k + 0.25 + jitter.y * 0.5) * spacing,
                );
                // Only once the bead has actually reached it, and dried by its
                // own age rather than by wherever we happen to be sampling.
                let sat_for = drop_pos.y * ky - sit.y;
                if sat_for > 0.0 && sat_for < persist {
                    let left_dried = sqrt(max(0.0, 1.0 - sat_for / persist));
                    let left_radius = radius * (0.16 + jitter.y * 0.22) * left_dried;
                    let here_in_screen = vec2(q.x, q.y * ky);
                    let clinging = smoothstep(left_radius, left_radius * 0.25,
                                              length(here_in_screen - sit))
                        * left_dried * life;
                    water = max(water, clinging);
                    runnel = max(runnel, clinging * 0.8);
                }
            }
        }
    }

    return vec2(water, runnel);
}

/// The screen, blurred by sampling around the point.
///
/// The usual trick is `textureLod` against a mip chain, which is nearly free,
/// but the post-process target is created with `mip_level_count: 1`, so there
/// is no chain to sample. Nine taps is the cheapest thing that still reads as
/// defocus rather than as a cross.
fn blurred(uv: vec2<f32>, radius: f32, aspect: f32) -> vec3<f32> {
    var sum = textureSample(screen_texture, texture_sampler, uv).rgb;
    if radius <= 0.0 {
        return sum;
    }
    for (var i = 0; i < 8; i++) {
        let a = f32(i) * 0.7853982;
        // Round on screen, not in UV, or the blur is an ellipse as wide as the
        // window.
        let o = vec2(cos(a) / aspect, sin(a)) * radius;
        sum += textureSample(screen_texture, texture_sampler,
                             clamp(uv + o, vec2(0.0), vec2(1.0))).rgb;
    }
    return sum / 9.0;
}

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let aspect = dims.x / max(dims.y, 1.0);

    // `drop_size` is a size, so it is the INVERSE of the grid frequency: the
    // more cells fit across the screen, the smaller each bead is. 64 keeps the
    // default (8) at eight cells across.
    let cells_across = 64.0 / max(settings.drop_size, 0.01);

    // One cell per column, each the full height of the screen. Cells a slice
    // tall meant a runner reached the bottom of its own cell an eighth of the
    // way down the screen and had to be faded out there, which is what made
    // them look like they were vanishing in mid-air.
    let g = vec2(in_uv.x * aspect * cells_across, in_uv.y);
    let t = settings.time * settings.speed;

    let here = wetness(g, t, cells_across);

    // The surface slope, by difference. Two extra evaluations, which is what
    // this costs over drawing each drop's distortion directly, and what buys
    // beads and runnels that bend light by the same rule.
    let e = 0.02;
    let dx = wetness(g + vec2(e, 0.0), t, cells_across).x - here.x;
    // Stepped in screen units too, or the vertical slope is `cols` times the
    // horizontal one and every drop refracts as though it were a wall.
    let dy = wetness(g + vec2(0.0, e / cells_across), t, cells_across).x - here.x;
    let slope = vec2(dx, dy);

    // Light bends down the slope, so the steeper the water the further the
    // image behind it moves. Gently: a bead 20 pixels across that shifts the
    // image by 18 shows a different part of the scene entirely, which is what
    // gave these the look of glass eyes rather than water.
    let offset = vec2(-slope.x / aspect, -slope.y) * settings.intensity * 0.15;
    let uv = clamp(in_uv + offset, vec2(0.0), vec2(1.0));

    // Glass in the rain is not clear glass: it fogs, and the water running down
    // it wipes the fog away. That contrast (a soft grey frame with sharp
    // streaks cut through it) is what reads as a wet windscreen. Beads on an
    // otherwise perfect image only ever read as marks on the picture.
    //
    // Runnels clear it hardest; a bead clears the spot it sits on.
    let cleared = max(here.x, here.y * 1.2);
    let blur = settings.fog * 0.006 * (1.0 - smoothstep(0.05, 0.5, cleared));
    var color = blurred(uv, blur, aspect);

    // Water is not a light source, so what it catches is a fraction of what is
    // already behind it: a bead over a lit sign glints, one over a dark alley
    // stays dark. Added as flat white it painted grey commas across the frame,
    // brightest exactly where the scene was darkest.
    //
    // The slope gives the shading for free too: water tilted away from the eye
    // darkens, water tilted toward it catches the light.
    let facing = slope.x * 0.7 - slope.y * 0.7;
    color *= 1.0 + clamp(facing * 7.0, -0.3, 0.4) * settings.intensity;

    return vec4(color, 1.0);
}
