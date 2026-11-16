-- Transient progress for long background jobs, shown on the message line.
-- Kept snacks-free so headless code can require it without a picker.
local M = {}

local THROTTLE_MS = 200
local BAR_WIDTH = 20

local state = nil

local function render()
  if not state then
    return
  end
  local total = state.total
  local fraction = total > 0 and state.done / total or 0
  local filled = math.floor(BAR_WIDTH * fraction + 0.5)
  local bar = string.rep("█", filled) .. string.rep("░", BAR_WIDTH - filled)
  local percent = math.floor(fraction * 100 + 0.5)
  local text = ("%s [%s] %d%% (%d/%d)"):format(state.label, bar, percent, state.done, total)
  vim.api.nvim_echo({ { text, "Title" } }, false, {})
end

--- Short progress string for a statusline, or `""` when idle.
---@return string
function M.status()
  if not state then
    return ""
  end
  local percent = state.total > 0 and math.floor(100 * state.done / state.total + 0.5) or 0
  return ("%s %d%%"):format(state.label, percent)
end

---@param label string
function M.start(label)
  state = { label = label, done = 0, total = 0, shown = vim.uv.hrtime() / 1e6 }
  render()
end

---@param done integer
---@param total integer
function M.update(done, total)
  if not state then
    state = { label = "mesh", done = 0, total = 0, shown = 0 }
  end
  state.done, state.total = done, total
  local now = vim.uv.hrtime() / 1e6
  if (total > 0 and done >= total) or now - state.shown >= THROTTLE_MS then
    state.shown = now
    render()
  end
end

--- Clear the message line once the job is done.
function M.finish()
  if not state then
    return
  end
  state = nil
  vim.api.nvim_echo({ { "" } }, false, {})
end

return M
