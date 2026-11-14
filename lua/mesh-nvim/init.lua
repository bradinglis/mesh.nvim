local util = require "mesh-nvim.util"
local create = require "mesh-nvim.create"
local buf = require "mesh-nvim.buf"
local bin = require "mesh-nvim.binary"
local find = require "mesh-nvim.find"
local pick = require "mesh-nvim.pick"
local config = require "mesh-nvim.config"
local comp = require "mesh-nvim.completion"
local embed = require "mesh-nvim.embed"

local M = {}

--- The basename of the current buffer without its extension.
---@return string
local function current_note_id()
  return vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(0)), "%.")[1]
end

---@param opts? opts
M.setup = function(opts)
  config.set_opts(opts)

  M.update()
  comp.register_providers()
  embed.warm()

  local group = vim.api.nvim_create_augroup("mesh", { clear = true })

  vim.api.nvim_create_autocmd("BufEnter", {
    group = group,
    pattern = "*.md",
    callback = function()
      comp.inject_sources()
      buf.highlights()
    end,
  })

  vim.api.nvim_create_autocmd("InsertLeave", {
    group = group,
    pattern = "*.md",
    callback = buf.highlights,
  })

  vim.api.nvim_create_autocmd("BufWritePost", {
    group = group,
    pattern = "*.md",
    callback = function()
      M.update()
      embed.schedule()
    end,
  })
end

local main_thread_channel = vim.uv.new_async(vim.schedule_wrap(function(run, opts)
  require("mesh-nvim.rfun").run(run, opts)
end))

local function update_worker(async_handle, dir, db, release_dir)
  local cpath = release_dir .. "/lib?.so"
  if not package.cpath:find(cpath, 1, true) then
    package.cpath = cpath .. ";" .. package.cpath
  end

  for _, value in ipairs(require("mesh").update_index(dir, db)) do
    async_handle:send(tostring(value.run), tostring(value.opts))
  end
end

M.update_async = function()
  local success
  success, M.update_thread = pcall(
    vim.uv.new_thread,
    update_worker,
    main_thread_channel,
    config.current_vault.dir,
    config.current_vault.db,
    util.get_paths().release_dir
  )

  if not success then
    vim.notify("Failed to spawn thread: " .. tostring(M.update_thread), vim.log.levels.ERROR)
  end
end

M.update = function()
  require("mesh-nvim.rfun").try(function()
    return bin.update_index(config.current_vault.dir, config.current_vault.db)
  end)
end

function M.annotations()
  local current_file = vim.api.nvim_buf_get_name(0)
  local note_id = current_note_id()

  require("snacks").picker.files({
    cwd = vim.fs.joinpath(config.current_vault.dir, "annotated"),
    confirm = function(picker, item)
      picker:close()

      util.try(function()
        return bin.get_annotations(item._path, current_file)
      end, function(results)
        for _, value in ipairs(results) do
          require("mesh-nvim.rfun").run(value.run, value.opts)
        end

        local asset_path = vim.fs.joinpath(
          config.current_vault.dir,
          "annotated",
          note_id .. ".pdf"
        )
        if vim.fn.rename(item._path, asset_path) ~= 0 then
          vim.notify("failed to move " .. item._path, vim.log.levels.ERROR)
        end
      end)
    end,
  })
end

M.create = create
M.refresh_embeddings = embed.refresh
M.find = find
M.buf = buf
M.pick = pick

M.open_asset = function()
  local base = vim.fs.joinpath(config.current_vault.dir, "annotated", current_note_id())

  for _, extension in ipairs({ ".pdf", ".epub" }) do
    local path = base .. extension
    if vim.fn.filereadable(path) == 1 then
      vim.fn.jobstart({ "lektra", path })
      return
    end
  end

  vim.notify("no asset found", vim.log.levels.WARN)
end

function M.fill_from_boox()
  local function get_file_contents(file_path)
    local file = io.open(file_path, "r")
    if not file then
      vim.notify("Error: Cannot open file " .. file_path, vim.log.levels.ERROR)
      return nil
    end

    local contents = file:read("*a")
    io.close(file)
    return contents
  end

  local current_buf = vim.api.nvim_get_current_buf()
  local current_id = current_note_id()

  require("snacks").picker.files({
    cwd = vim.fs.joinpath(config.current_vault.dir, "boox"),
    confirm = function(self, item)
      local content = get_file_contents(item._path)
      if not content then
        self:close()
        return
      end

      local chunks = { {} }

      for _, value in ipairs(vim.split(content, "\n")) do
        value = value:gsub(".*Page No.", "Page No.")
        if value == "-------------------" then
          table.insert(chunks, {})
        elseif value:match("^Page") ~= nil then
          chunks[#chunks].page = value:gsub("Page No.: ", "")
        elseif value:match("^【Annotation】") ~= nil then
          chunks[#chunks].annotation = value:gsub("【Annotation】", "")
        elseif chunks[#chunks].annotation ~= nil then
          chunks[#chunks].annotation = chunks[#chunks].annotation .. " " .. value
        elseif chunks[#chunks].text ~= nil then
          chunks[#chunks].text = chunks[#chunks].text .. " " .. value
        elseif chunks[#chunks].page ~= nil then
          chunks[#chunks].text = value
        end
      end

      chunks = vim.tbl_map(function(chunk)
        chunk.text = chunk.text or ""
        if chunk.page and chunk.page ~= "" then
          chunk.text = chunk.text .. " (p." .. chunk.page .. ")"
        end

        chunk.text = chunk.text
          :gsub("\r", "")
          :gsub("%—", "--")
          :gsub("[%‘%’]+", "%'")
          :gsub("  ", " ")
          :gsub("%-%-", "—")

        local result = {}
        if chunk.annotation then
          table.insert(result, chunk.annotation .. ":")
        end
        table.insert(result, "> " .. chunk.text)
        table.insert(result, "")
        return result
      end, chunks)

      local name = vim.split(item._path, "-annotation")[1]

      local function copy_asset(extension, label)
        local source = name .. extension
        if vim.fn.filereadable(source) ~= 1 then
          vim.notify("no " .. extension:sub(2))
          return true
        end

        local destination =
          vim.fs.normalize(vim.fs.joinpath(config.current_vault.dir, "annotated", current_id .. extension))
        if vim.fn.filecopy(source, destination) ~= 1 then
          vim.notify("failure to copy", vim.log.levels.ERROR)
          return false
        end

        vim.fn.delete(source)
        table.insert(chunks, 1, {
          ("[[%s%s|Annotated %s]]"):format(current_id, extension, label),
          "",
        })
        return true
      end

      if not copy_asset(".pdf", "PDF") or not copy_asset(".epub", "EPUB") then
        return
      end

      local lines = vim.iter(chunks):flatten():totable()
      vim.api.nvim_buf_set_lines(current_buf, -1, -1, false, lines)
      vim.fn.delete(item._path)
      self:close()
    end,
  })
end

return M
