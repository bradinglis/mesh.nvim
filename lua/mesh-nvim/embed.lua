-- Background embedding refresh and model warm-up. These run in their own worker
-- threads and are deliberately never joined by util.try, so pickers stay
-- responsive while a large vault is embedded.
local util = require "mesh-nvim.util"
local config = require "mesh-nvim.config"

local M = {}

local running = false
local pending = false
local scheduled = false

-- Messages from the worker arrive as strings and are handled on the main thread.
local channel = vim.uv.new_async(vim.schedule_wrap(function(run, opts)
  if run == "done" then
    running = false
    if pending then
      pending = false
      M.refresh()
    end
    return
  end
  if run == "log" and opts:match("^Embeddings: 0 written, 0 removed") then
    return
  end
  require("mesh-nvim.rfun").run(run, opts)
end))

---@param handle userdata async handle back to the main thread
---@param release_dir string
---@param action "warm"|"update"
---@param dir string
---@param db string
local function worker(handle, release_dir, action, dir, db)
  local cpath = release_dir .. "/lib?.so"
  if not package.cpath:find(cpath, 1, true) then
    package.cpath = cpath .. ";" .. package.cpath
  end

  local ok, err = pcall(function()
    local mesh = require "mesh"
    local actions
    if action == "warm" then
      actions = mesh.warm_model(db)
    else
      actions = mesh.update_embeddings(dir, db)
    end
    for _, value in ipairs(actions) do
      handle:send(tostring(value.run), tostring(value.opts))
    end
  end)
  if not ok then
    handle:send("err", tostring(err))
  end
  handle:send("done", "")
end

---@param action "warm"|"update"
local function spawn(action)
  running = true
  local current = config.current_vault
  local ok, thread = pcall(
    vim.uv.new_thread,
    worker,
    channel,
    util.get_paths().release_dir,
    action,
    current.dir,
    current.db
  )
  if not ok then
    running = false
    vim.notify("Failed to start embedding thread: " .. tostring(thread), vim.log.levels.ERROR)
  end
end

--- Load the embedding model in the background so the first search is fast.
function M.warm()
  if not running then
    spawn("warm")
  end
end

--- Bring stored embeddings up to date with the vault. Safe to call often: if a
--- run is already in progress, exactly one follow-up run is queued.
function M.refresh()
  if running then
    pending = true
    return
  end
  spawn("update")
end

--- Debounced refresh, used after saves.
---@param delay? integer milliseconds
function M.schedule(delay)
  if scheduled then
    return
  end
  scheduled = true
  vim.defer_fn(function()
    scheduled = false
    M.refresh()
  end, delay or 1000)
end

return M
