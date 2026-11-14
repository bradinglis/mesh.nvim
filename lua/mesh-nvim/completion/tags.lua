local bin = require "mesh-nvim.binary"
local config = require "mesh-nvim.config"
local util = require "mesh-nvim.util"
local completion = require "mesh-nvim.completion"

return completion.module(function(ctx, callback)
  --- @type lsp.CompletionItem[]
  local items = {}

  local is_frontmatter = ctx.line:sub(1, 5) == "tags:"
  local intext = ctx.line:sub(1, 1) == "#"
    or ctx.line:sub(ctx.bounds.start_col - 2, -1) == " #"

  if not is_frontmatter and not intext then
    return completion.respond(callback, items)
  end

  local prefix = is_frontmatter and "" or "#"
  local start_offset = is_frontmatter and 1 or 2
  local kind = require("blink.cmp.types").CompletionItemKind.Field

  util.try(function()
    return bin.get_tags(config.current_vault.dir, config.current_vault.db)
  end, function(tags)
    for _, tag in ipairs(tags) do
      items[#items + 1] = {
        label = tag.tag,
        filterText = prefix .. tag.tag,
        kind = kind,
        textEdit = {
          newText = prefix .. tag.tag,
          range = {
            start = {
              line = ctx.bounds.line_number - 1,
              character = ctx.bounds.start_col - start_offset,
            },
            ["end"] = {
              line = ctx.bounds.line_number - 1,
              character = ctx.bounds.start_col + ctx.bounds.length - 1,
            },
          },
        },
        insertTextFormat = vim.lsp.protocol.InsertTextFormat.PlainText,
      }
    end
  end)

  return completion.respond(callback, items)
end)
