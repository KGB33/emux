---@class Locator A pipeline of file filters.
---@field filters table[]

---@class Overrider A value-replacement strategy.

---@class EmuxLocators
---@field envFile fun(path: string, variable: string): Locator Target a variable in a dotenv-style file.
---@field files fun(glob: string): Locator Locate files matching a glob pattern.
---@field regex fun(target: Locator, pattern: string): Locator Chain a regex filter onto a locator pipeline.

---@class EmuxOverriders
---@field randPort Overrider Replace located values with a randomly generated free port.

---@class EmuxLib The emux API available in all config files.
---@field l EmuxLocators Locator functions for finding values in files.
---@field o EmuxOverriders Overrider values for replacing located values.

---@type EmuxLib
local emux = {}

emux.l = {
    envFile = function(path, variable)
        return { filters = { { __kind = "env_file", path = path, variable = variable } } }
    end,
    files = function(glob)
        return { filters = { { __kind = "file", glob = glob } } }
    end,
    regex = function(target, pattern)
        local filters = {}
        for _, f in ipairs(target.filters) do table.insert(filters, f) end
        table.insert(filters, { __kind = "regex", pattern = pattern })
        return { filters = filters }
    end,
}

emux.o = {
    randPort = { __kind = "random_port" },
}

return emux
