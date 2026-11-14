local bin = require "mesh-nvim.binary"
local config = require "mesh-nvim.config"
local util = require "mesh-nvim.util"
local completion = require "mesh-nvim.completion"

return completion.module(function(ctx, callback)
  --- @type lsp.CompletionItem[]
  local items = {}

  local check = string.match(
    string.sub(ctx.line, 0, ctx.bounds.start_col - 1),
    "%[%[([^]]*)#[^]]*$"
  )

  if check ~= nil then
    local length = string.find(ctx.line, "%]", ctx.bounds.start_col)
    local kind = require("blink.cmp.types").CompletionItemKind.Reference

    util.try(function()
      return bin.get_note_anchors(config.current_vault.dir, config.current_vault.db, check)
    end, function(anchors)
      for _, anchor in ipairs(anchors) do
        items[#items + 1] = {
          label = anchor.title,
          filterText = anchor.title .. " " .. anchor.id,
          kind = kind,
          textEdit = {
            newText = "[[" .. anchor.id .. "$1|${2:" .. anchor.title .. "}]]$0",
            range = {
              start = {
                line = ctx.bounds.line_number - 1,
                character = ctx.bounds.start_col - check:len() - 4,
              },
              ["end"] = {
                line = ctx.bounds.line_number - 1,
                character = length + 1,
              },
            },
          },
          insertTextFormat = vim.lsp.protocol.InsertTextFormat.Snippet,
        }
      end
    end)
  end

  return completion.respond(callback, items)
end)
