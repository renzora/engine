-- A live counter that UI templates can bind with {{ EntityName.score }}.
-- Demonstrates: exposing runtime state to templates via props(), accumulating
-- in on_update with delta.
--
-- Name the entity (e.g. "Scoreboard"), then in a template:
--   <text>Score: {{ Scoreboard.score }}</text>

function props()
    return {
        score      = { type = "int", value = 0 },
        per_second = { type = "int", value = 10, hint = "points added per second" },
    }
end

local accumulator = 0.0

function on_update()
    accumulator = accumulator + per_second * delta
    if accumulator >= 1.0 then
        local add = math.floor(accumulator)
        score = score + add
        accumulator = accumulator - add
    end
end
