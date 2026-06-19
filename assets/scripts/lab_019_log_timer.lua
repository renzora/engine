-- lab_019_log_timer: starts a repeating timer and logs each time it fires.
-- Demonstrates the start_timer + timers_finished pattern: we scan the finished
-- list every frame rather than assuming the timer fired, since several may
-- complete on the same frame.

function props()
    return {
        interval = { value = 2.0, hint = "Seconds between log lines", tab = "Timer" },
    }
end

function on_ready()
    start_timer("heartbeat", interval, true)
end

function on_update()
    for _, name in ipairs(timers_finished) do
        if name == "heartbeat" then
            print_log("heartbeat from " .. self_entity_name)
        end
    end
end
