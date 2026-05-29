-- Logs into renzora.com and exposes the username for UI templates to bind.
--
-- Setup:
--   1. Put this script on an entity NAMED "Account" (so templates can read
--      `{{ Account.username }}`).
--   2. Fill in email + password in the inspector (props below).
--   3. A UiCanvas + HtmlTemplate -> templates/account.html displays the result.
--
-- Flow: on_ready POSTs to the login API; the async response arrives at
-- on_http, where we json_parse the body and stash username / login_status.
-- Those are props (persisted variables), so the binding system reads them.

function props()
    return {
        email        = { type = "string", value = "",              hint = "renzora.com email" },
        password     = { type = "string", value = "",              hint = "password" },
        username     = { type = "string", value = "(not logged in)", hint = "Internal: result" },
        login_status = { type = "string", value = "idle",          hint = "Internal: status" },
    }
end

function on_ready()
    if email == "" then
        login_status = "set email + password"
        return
    end
    login_status = "logging in..."
    -- Simple JSON body. (For values with quotes you'd want proper escaping;
    -- fine for a demo login.)
    local body = '{"email":"' .. email .. '","password":"' .. password .. '"}'
    http_post("https://renzora.com/api/auth/login", body, "login")
end

-- Called when an http request finishes. name = the callback you passed.
function on_http(name, code, response)
    if name ~= "login" then return end

    if code == 200 then
        local data = json_parse(response)
        if data and data.user and data.user.username then
            username = data.user.username
            login_status = "online"
            print("[account] logged in as " .. username)
        else
            login_status = "ok but no username in response"
        end
    else
        login_status = "login failed (" .. code .. ")"
        print("[account] login failed " .. code .. ": " .. tostring(response))
    end
end
