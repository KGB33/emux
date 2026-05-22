---@meta

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
---@field l EmuxLocators Locator functions.
---@field o EmuxOverriders Overrider values.

---@class _G
---@field emux EmuxLib
