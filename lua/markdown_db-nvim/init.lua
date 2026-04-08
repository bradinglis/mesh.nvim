local util = require "markdown_db-nvim.util"
local create = require "markdown_db-nvim.create"
local buf = require "markdown_db-nvim.buf"
local bin = require "markdown_db-nvim.binary"
local find = require "markdown_db-nvim.find"
local pick = require "markdown_db-nvim.pick"
local config = require "markdown_db-nvim.config"
local comp = require "markdown_db-nvim.completion"

local M = {}

M.setup = function(opts)
  config.set_opts(opts)

  M.update()
  comp.register_providers()

  vim.api.nvim_create_autocmd("BufEnter", {
    pattern = "*.md",
    callback = function()
      comp.inject_sources()
      M.buf.highlights()
    end,
  })

  vim.api.nvim_create_autocmd("InsertLeave", {
    pattern = "*.md",
    callback = function()
      M.buf.highlights()
    end,
  })

  vim.api.nvim_create_autocmd("BufWritePost", {
    pattern = "*.md",
    callback = function()
      M.update()
    end,
  })
end

M.update = function()
  require("markdown_db-nvim.rfun").try(function()
      return bin.update_index(config.current_vault.dir, config.current_vault.db)
    end
  )
end

M.create = create
M.find = find
M.buf = buf
M.pick = pick
M.open_asset = function()
  local asset_path = config.current_vault.dir ..
  "/annotated/" .. vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(0)), "%.")[1]

  if vim.fn.filereadable(asset_path .. ".pdf") == 1 then
    asset_path = asset_path .. ".pdf"
  elseif vim.fn.filereadable(asset_path .. ".epub") == 1 then
    asset_path = asset_path .. ".epub"
  else
    vim.notify("no file")
  end

  vim.fn.jobstart({ "lektra", asset_path })
end




function M.fill_from_boox()
  local function get_file_contents(file_path)
    local file = io.open(file_path, "r")
    if file then
      local contents = file:read("*a")
      io.close(file)
      return contents
    else
      print("Error: Cannot open file " .. file_path)
      return nil
    end
  end


  local current_buf = vim.api.nvim_get_current_buf()
  local current_id = vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(current_buf)), "%.")[1]

  require("snacks").picker.files({
    cwd = config.current_vault.dir .. "/boox",
    confirm = function(self, item)
      local content = get_file_contents(item._path)
      if not content then
        self:close()
        return
      end

      local chunks = { {} }

      local lines = vim.split(content, "\n")
      for _, value in ipairs(lines) do
        value = value:gsub(".*Page No.", "Page No.")
        if value == "-------------------" then
          table.insert(chunks, {})
        elseif value:match("^Page") ~= nil then
          chunks[#chunks].page = value:gsub("Page No.: ", "")
        elseif value:match("^【Annotation】") ~= nil then
          chunks[#chunks].annotation = value:gsub("【Annotation】", "")
        elseif chunks[#chunks].annotation ~= nil then
          chunks[#chunks].annotation = (chunks[#chunks].annotation .. " " .. value) or value
        elseif chunks[#chunks].text ~= nil then
          chunks[#chunks].text = chunks[#chunks].text .. " " .. value
        elseif chunks[#chunks].page ~= nil then
          chunks[#chunks].text = value
        end
      end
      vim.notify(chunks)

      chunks = vim.tbl_map(function(chunk)
        local res = {}

        if chunk.page ~= "" and chunk.page ~= nil then
          chunk.text = chunk.text .. " (p." .. chunk.page .. ")"
        end

        if chunk.text == nil then
          chunk.text = ""
        end
        chunk.text = chunk.text:gsub("\r", "")
            :gsub("%—", "--")
            :gsub("[%‘%’]+", "%'")
            :gsub("  ", " ")
            :gsub("%-%-", "—")

        if chunk.annotation ~= nil then
          table.insert(res, chunk.annotation .. ":")
        end
        table.insert(res, "> " .. chunk.text)
        table.insert(res, "")
        return res
      end, chunks)

      local name = vim.split(item._path, "-annotation")[1]

      local possible_pdf = name .. ".pdf"
      if vim.fn.filereadable(possible_pdf) == 1 then
        if vim.fn.filecopy(possible_pdf, vim.fs.normalize(config.current_vault.dir .. "/annotated/" .. current_id .. ".pdf")) == 1 then
          vim.fn.delete(possible_pdf)
          table.insert(chunks, 1, { "[[" .. current_id .. ".pdf|Annotated PDF]]", "" })
        else
          vim.notify("failure to copy")
          return
        end
      else
        vim.notify("no pdf")
      end

      local possible_epub = name .. ".epub"
      if vim.fn.filereadable(possible_epub) == 1 then
        if vim.fn.filecopy(possible_epub, vim.fs.normalize(config.current_vault.dir .. "/annotated/" .. current_id .. ".epub")) == 1 then
          vim.fn.delete(possible_epub)
          table.insert(chunks, 1, { "[[" .. current_id .. ".epub|Annotated EPUB]]", "" })
        else
          vim.notify("failure to copy")
          return
        end
      else
        vim.notify("no epub")
      end

      content = vim.iter(chunks):flatten():totable()
      vim.api.nvim_buf_set_lines(current_buf, -1, -1, false, content)
      vim.fn.delete(item._path)
      self:close()
    end
  })
end

return M
