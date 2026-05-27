-- Networked Transform — server-driven motion that replicates to clients.
--
-- The SERVER moves this entity in a circle; its Transform replicates to every
-- client with smooth interpolation. Server-authoritative: clients don't drive
-- it, they receive it.
--
-- SETUP: attach to an entity that ALSO exists in the loaded scene on every peer
-- (e.g. a cube you placed in the scene), and add the `Networked` component to
-- it in the inspector. Then watch the cube track the server's motion on the
-- clients.
--
-- NOTE: today Transform *data* replicates, but a server-only-spawned entity's
-- VISUAL doesn't arrive on clients yet (that's Phase 4 prefab-spawn
-- replication, not built). So test on an entity present in the scene on all
-- peers — the position will sync.

function props()
    return {
        radius = { type = "float", value = 3.0, hint = "Circle radius" },
        speed  = { type = "float", value = 1.0, hint = "Radians/sec" },
    }
end

function on_update()
    if net_is_server() then
        local a = elapsed * speed
        set_position(math.cos(a) * radius, 1.0, math.sin(a) * radius)
    end
end
