-- renzora.com login — driven by a UI form.
--
-- Put this on an entity NAMED "Account". The login form's <input> fields bind
-- to this script's `email` / `password` vars; the "Log in" button fires
-- on_ui("submit_login"), which POSTs them and stores the username.
--
-- Pairs with templates/login_form.html on a UiCanvas.

function props()
    return {
        email        = { type = "string", value = "" },
        password     = { type = "string", value = "" },
        username     = { type = "string", value = "" },
        login_status = { type = "string", value = "idle" },
    }
end

function on_ui(name)
    if name ~= "submit_login" then return end
    if email == "" or password == "" then
        login_status = "enter email + password"
        return
    end
    login_status = "logging in..."
    local body = '{"email":"' .. email .. '","password":"' .. password .. '"}'
    http_post("https://renzora.com/api/auth/login", body, "login")
end

function on_http(name, code, body)
    if name ~= "login" then return end
    if code == 200 then
        local data = json_parse(body)
        if data and data.user and data.user.username then
            username = data.user.username
            login_status = "online"
            print("[account] logged in as " .. username)
        else
            login_status = "ok but no username in response"
        end
    else
        login_status = "failed (" .. code .. ")"
        print("[account] login failed " .. code .. ": " .. tostring(body))
    end
end
