-- Ragdoll trigger demo.
-- Attach to the SAME entity that carries the `Ragdoll` component (the model
-- root, alongside `Animator`) — `enable_ragdoll()` / `disable_ragdoll()` always
-- act on the script's own entity.
--
-- IMPORTANT: ragdoll is a physics feature, so it only does anything in PLAY mode
-- (the editor pauses the simulation). It also waits for the skeleton to finish
-- loading, which is why this fires on a short delay rather than instantly.
--
-- Behaviour: ~DROP_DELAY seconds after Play starts the character collapses on
-- its own (no input needed). Press the recover key to stand it back up.
--
-- Demonstrates: props(), on_ready, on_update, the `delta` global,
-- is_key_just_pressed(), enable_ragdoll()/disable_ragdoll().

-- Seconds after Play before the ragdoll fires. A delay (not an instant trigger
-- in on_ready) matters: the GLB skeleton and its physics bodies spawn a few
-- frames after the scene loads, so triggering too early would no-op.
local DROP_DELAY = 2.0

function props()
    return {
        recover_key = { type = "string", value = "KeyG", hint = "key to return to animation" },
    }
end

-- Local state so we only fire each transition once, on the edge.
local ragdolling = false
local elapsed = 0.0

function on_ready()
    ragdolling = false
    elapsed = 0.0
    print("[ragdoll_demo] Play mode: collapsing in " .. DROP_DELAY .. "s; press " .. recover_key .. " to recover")
end

function on_update()
    if not ragdolling then
        elapsed = elapsed + delta
        if elapsed >= DROP_DELAY then
            enable_ragdoll()        -- bones -> Dynamic, animator detaches + pauses
            ragdolling = true
            print("[ragdoll_demo] ragdoll enabled")
        end
        return
    end

    if is_key_just_pressed(recover_key) then
        disable_ragdoll()           -- bones -> Kinematic, animation resumes
        ragdolling = false
        elapsed = 0.0
        print("[ragdoll_demo] ragdoll disabled (animation resumed)")
    end
end
