--- Lua module docs.
-- Module context that should stay.
local M = {}

function M:greet(name)
  --- Normalize before returning.
  local function normalize(value)
    -- Leave the nested exit visible.
    return value:gsub("^%s+", "")
  end

  return normalize(name)
end

M.build = function(prefix)
  -- Reject empty prefixes.
  if prefix == "" then
    return nil
  end

  return function(name)
    return prefix .. ": " .. name
  end
end

M.handlers = {
  format = function(name)
    -- Keep the table-field exit.
    return name .. "!"
  end,
}

return M
