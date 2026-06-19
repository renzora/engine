-- lab_014_look_at_origin: continuously faces the world origin.
-- Re-aiming every frame means that if the entity is moved (by a parent, physics,
-- or another script) it keeps pointing at (0,0,0) without any manual angle math.

function on_update()
    look_at(0.0, 0.0, 0.0)
end
