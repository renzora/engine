-- lab_016_random_phase_bob: vertical bob with a per-instance random phase.
-- Seeding the phase once on ready means many copies of this script bob out of
-- sync, breaking the "everything moves as one" look without per-entity tuning.

local base_y = 0.0
local phase = 0.0

function props()
    return {
        amplitude = { value = 0.4, hint = "Bob height", tab = "Bob" },
        frequency = { value = 1.0, hint = "Cycles per second", tab = "Bob" },
    }
end

function on_ready()
    base_y = position_y
    -- Seed from the entity id so each instance is deterministic yet distinct.
    math.randomseed(self_entity_id + math.floor(elapsed * 1000.0))
    phase = math.random() * math.pi * 2.0
end

function on_update()
    local y = math.sin(elapsed * frequency * math.pi * 2.0 + phase) * amplitude
    set_position(position_x, base_y + y, position_z)
end
