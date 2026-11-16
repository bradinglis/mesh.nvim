local snacks_picker = require "snacks.picker"
local conf = require "mesh-nvim.config"

local M = {}

local icon_dict = {
  author = "",
  note = "",
  source = "",
  journal = "",
}

local link_dict = {
  forward = {
    intext_link = "Links to",
    frontmatter_reference = "References",
    author = "Author",
    parent = "Parent",
  },
  back = {
    intext_link = "Linked by",
    frontmatter_reference = "Referenced by",
    author = "Source",
    parent = "Child",
  },
}


local picker_opts = {
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
  previewers = {
    file = {
      max_line_length = 5000, -- max line length
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

local ELLIPSIS = "…"

--- Fit `text` to exactly `width` display columns, truncating with an ellipsis
--- when needed so padded and truncated fields still line up.
local function set_string_width(text, width)
  text = text or ""
  local text_width = vim.fn.strdisplaywidth(text)

  if text_width <= width then
    return text .. string.rep(" ", width - text_width)
  end

  local budget = width - vim.fn.strdisplaywidth(ELLIPSIS)
  local chars = {}
  local used = 0
  local index = 0
  while used < budget do
    local char = vim.fn.strcharpart(text, index, 1)
    if char == "" then
      break
    end
    local char_width = vim.fn.strdisplaywidth(char)
    if used + char_width > budget then
      break
    end
    chars[#chars + 1] = char
    used = used + char_width
    index = index + 1
  end

  local truncated = table.concat(chars) .. ELLIPSIS
  local truncated_width = vim.fn.strdisplaywidth(truncated)
  if truncated_width < width then
    truncated = truncated .. string.rep(" ", width - truncated_width)
  end
  return truncated
end

function M.from_notes(notes)
  local i = #notes
  local entries = vim.tbl_map(function(n)
    i = i - 1
    local title = n.title or ""
    local authors = table.concat(n.authors or {}, ", ")
    local text = n.text or (title .. " " .. authors .. " " .. (n.id or ""))

    return {
      text = text,
      ordinal = text,
      title = title,
      author = authors,
      kind = n.kind,
      grep_text = n.text or nil,
      direction = n.direction,
      tags = n.tags or {},
      icon = icon_dict[n.type] or "",
      score_add = (i / 30),
      file = n.path,
      id = n.id,
      pos = { n.line or 0, n.start_col or 0 },
      end_pos = { n.line or 0, n.end_col or 0 },
      link = "[[" .. (n.id or "") .. "|" .. title .. "]]",
    }
  end, notes)

  local notes_opts = vim.tbl_deep_extend("force", picker_opts, {
    title = "Notes",
    items = entries,
    format = function(item, _)
      local ret = {}

      ret[#ret + 1] = { set_string_width(item.icon, 2) .. " ", "Fg" }
      if item.kind and item.direction then
        local label = (link_dict[item.direction] or {})[item.kind] or item.kind
        ret[#ret + 1] = { set_string_width(label, 14) .. " ", "Fg" }
      end

      if item.grep_text then
        ret[#ret + 1] = { set_string_width("[" .. item.pos[1] .. "]", 5), "Fg" }
        ret[#ret + 1] = { set_string_width(item.title, 40) .. " ", "markdownBoldItalic" }
        ret[#ret + 1] = { item.grep_text, "Fg" }
      else
        if item.author ~= "" then
          ret[#ret + 1] = { set_string_width(item.title, 40) .. " ", "markdownBoldItalic" }
          ret[#ret + 1] = { set_string_width(item.author, 20) .. " ", "markdownItalic" }
        else
          ret[#ret + 1] = { set_string_width(item.title, 61) .. " ", "markdownBoldItalic" }
        end

        ret[#ret + 1] = { set_string_width(table.concat(item.tags, " "), 60) .. " ", "ObsidianTag" }
        ret[#ret + 1] = { item.id, "Grey" }
      end

      return ret
    end,
  })

  snacks_picker.pick(notes_opts)
end

function M.from_tags(tags)
  local entries = vim.iter(tags):map(function(tag)
    return {
      text = tag.tag .. tag.count,
      number = tag.count,
      notes = table.concat(vim.tbl_map(function(n) return n.id end, tag.notes or {}), " "),
      score_add = tag.count,
      tag = tag.tag,
      data = tag
    }
  end):totable()

  local pick_opts = {
    title = "Tags",
    items = entries,
    preview = "none",
    layout = {
      layout = {
        backdrop = false,
        width = 0.90,
        min_width = 80,
        height = 0.95,
        border = "none",
        box = "vertical",
        {
          box = "vertical",
          border = "rounded",
          title = "{title} {live} {flags}",
          title_pos = "center",
          { win = "input", height = 1,     border = "bottom" },
          { win = "list",  border = "none" },
        },
      }
    },
    format = function(item, _)
      local ret = {}
      ret[#ret + 1] = { set_string_width("[" .. item.number .. "] ", 5) .. " ", "Fg" }
      ret[#ret + 1] = { set_string_width(item.tag, 40) .. " ", "ObsidianTag" }
      ret[#ret + 1] = { item.notes, "Grey" }
      return ret
    end,
    confirm = function(picker, item)
      require("mesh-nvim.find").tag_notes(item.tag)
      picker:close()
    end,
  }
  snacks_picker.pick(pick_opts)
end

function M.vault()
  local entries = {}
  for name, vault in pairs(conf.config.vaults) do
    if name ~= "default" or type(vault) == "table" then
      local vault_table = type(vault) == "table" and vault or { name = name, dir = vault }
      entries[#entries + 1] = {
        text = vault_table.name or name,
        name = vault_table.name or name,
        dir = vault_table.dir,
        value = vault_table,
      }
    end
  end

  local pick_opts = {
    title = "Vaults",
    items = entries,
    preview = "none",
    layout = {
      layout = {
        backdrop = false,
        width = 0.90,
        min_width = 80,
        height = 0.95,
        border = "none",
        box = "vertical",
        {
          box = "vertical",
          border = "rounded",
          title = "{title} {live} {flags}",
          title_pos = "center",
          { win = "input", height = 1,     border = "bottom" },
          { win = "list",  border = "none" },
        },
      }
    },
    format = function(item, _)
      return { { item.text, "Fg" } }
    end,
    confirm = function(picker, item)
      conf.set_vault(item.name, item.dir)
      picker:close()
    end,
  }

  snacks_picker.pick(pick_opts)
end

---@param items table[] from `mesh-nvim.semantic`
---@param title string
function M.from_semantic(items, title)
  snacks_picker.pick({
    title = title,
    items = items,
    layout = picker_opts.layout,
    preview = "file",
    format = function(item, _)
      local ret = {}
      ret[#ret + 1] = { ("%.2f "):format(item.score), "Comment" }
      ret[#ret + 1] = { set_string_width(item.kind, 8) .. " ", "Fg" }
      ret[#ret + 1] = { set_string_width(item.title, 36) .. " ", "markdownBoldItalic" }
      if item.heading then
        ret[#ret + 1] = { set_string_width(item.heading, 28) .. " ", "markdownItalic" }
      end
      ret[#ret + 1] = { item.snippet, "Fg" }
      return ret
    end,
    confirm = function(picker, item)
      picker:close()
      require("mesh-nvim.buf").open({ path = item.path, row = item.row, col = item.col })
    end,
  })
end

return M
