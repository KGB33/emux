---@meta

---@class Locator A pipeline of file filters.
---@field filters table[]

---@class Overrider A value-replacement strategy.

---@class EmuxLocators
---@field envFile fun(path: string, variable: string): Locator Target a variable in a dotenv-style file.
---@field jsonFile fun(path: string, selector: string): Locator Target a value at a dotted selector in a JSON file.

---@class EmuxOverriders
---@field port Overrider Replace located values with a deterministic per-worktree free-range port.

---@class EmuxLib The emux API available in all config files.
---@field l EmuxLocators Locator functions.
---@field o EmuxOverriders Overrider values.

---@class _G
---@field emux EmuxLib
