local snacks_picker = require "snacks.picker"
local util = require "markdown_db-nvim.util"

local M = {}


local bin = {}

M.config = {
  vaults = {
    default = "none",
  }
}

M.current_vault = {
  name = "none",
  dir = "",
  db = ""
}

M.setup = function(opts)
  M.config = vim.tbl_extend("force", M.config, opts)

  bin = require("markdown_db-nvim.binary")

  local db_root = util.get_paths().data_dir
  vim.fn.mkdir(db_root, "p")

  if type(M.config.vaults.default) == "string" then
    M.current_vault.name = M.config.vaults[M.config.vaults.default].name
    M.current_vault.dir = M.config.vaults[M.config.vaults.default].dir
  elseif type(M.config.vaults.default) == "table" then
    M.current_vault.name = M.config.vaults.default.name
    M.current_vault.dir = M.config.vaults.default.dir
  end

  M.current_vault.db = db_root .. "/" .. M.current_vault.name .. ".duckdb"

  M.update()
  vim.api.nvim_create_autocmd("BufWritePost", {
    pattern = "*.md",
    callback = function()
      M.update()
    end,
  })
end

M.update = function()
  vim.notify(bin.update_index(M.current_vault.dir, M.current_vault.db))
end

local set_string_width = function(text, width)
  if (vim.fn.strdisplaywidth(text) > width) then
    return vim.fn.strcharpart(text, -1, width) .. "…"
  elseif vim.fn.strdisplaywidth(text) < width then
    return text .. string.rep(" ", width - vim.fn.strdisplaywidth(text))
  else
    return text
  end
end


local icon = {
  author = "",
  note = "",
  source = "",
}

M.find_notes = function()
  local notes = bin.get_notes(M.current_vault.dir, M.current_vault.db)
  dd(notes)

  local entries = vim.tbl_map(function(n)
    return {
      text = (n.title or "") .. " " .. (table.concat(n.authors, ", ") or "") .. " " .. n.id,
      ordinal = (n.title or "") .. " " .. (table.concat(n.authors, ", ") or "") .. " " .. n.id,
      title = (n.title or ""),
      author = (table.concat(n.authors, ", ") or ""),
      tags = n.tags,
      icon = icon[n.type] or "",
      -- score_add = 70 + ((n.note.recency * -1)/10),
      file = n.path,
      id = n.id,
      link = "[[" .. n.id .. "|" .. (n.title or "") .. "]]",
    }
  end, notes)


  local pick = {
    layout = {
      layout = {
        backdrop = false,
        width = 0.90,
        min_width = 80,
        height = 0.95,
        border = "none",
        box = "vertical",
        { win = "preview", title = "{preview}", height = 0.5, border = "single" },
        {
          box = "vertical",
          border = "single",
          title = "{title} {live} {flags}",
          title_pos = "center",
          { win = "input", height = 1,     border = "bottom" },
          { win = "list",  border = "none" },
        },
      }
    },
    win = {
      input = {
        keys = {
          ["<C-l>"] = { "put_link", mode = { "n", "i" }, desc = "Put Link" },
          ["<C-o>"] = { "put_id", mode = { "n", "i" }, desc = "Put ID" },
        },
      },
    },
    buffers = true,
    actions = {
      put_link = function(picker)
        local link = picker:current().link
        picker:close()
        vim.api.nvim_put({ link }, "", false, true)
      end,
      put_id = function(picker)
        local id = picker:current().id
        picker:close()
        vim.api.nvim_put({ id }, "", false, true)
      end,
    },
  }

  local pick_opts = vim.tbl_deep_extend('force', pick or {}, {
    title = "Notes",
    items = entries,
    format = function(item, _)
      if item.author ~= "" then
        return {
          { set_string_width(item.icon, 2) .. " ",                     "Fg" },
          { set_string_width(item.title, 40) .. " ",                   "markdownBoldItalic" },
          { set_string_width(item.author, 20) .. " ",                  "markdownItalic" },
          { set_string_width(table.concat(item.tags, " "), 60) .. " ", "ObsidianTag" },
          { item.id,                                                   "Grey" }
        }
      else
        return {
          { set_string_width(item.icon, 2) .. " ",                     "Fg" },
          { set_string_width(item.title, 60) .. " ",                   "markdownBoldItalic" },
          { set_string_width(table.concat(item.tags, " "), 60) .. " ", "ObsidianTag" },
          { item.id,                                                   "Grey" }
        }
      end
    end
  })

  snacks_picker.pick(pick_opts)
end

return M
