-- Fetches JSON from an API and exposes a field for UI binding.
-- Demonstrates: http_get, on_http, json_parse, surfacing the result as a prop.
--
-- Name the entity (e.g. "Api"), then bind in a template:
--   <text>{{ Api.result }}</text>

function props()
    return {
        url    = { type = "string", value = "https://renzora.com/api/health",
                   hint = "endpoint to GET" },
        result = { type = "string", value = "(loading)", hint = "Internal: response" },
    }
end

function on_ready()
    http_get(url, "fetch")
end

function on_http(name, code, body)
    if name ~= "fetch" then return end
    if code == 200 then
        -- Show the raw body, or pull a field:
        --   local data = json_parse(body)
        --   result = data and data.status or body
        result = body
    else
        result = "error " .. code
    end
end
