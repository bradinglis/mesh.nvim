local bin = require "mesh-nvim.binary"
local config = require "mesh-nvim.config"
local util = require "mesh-nvim.util"
local completion = require "mesh-nvim.completion"

return completion.module(function(ctx, callback)
  --- @type lsp.CompletionItem[]
  local items = {}

  local check = string.match(string.sub(ctx.line, 0, ctx.bounds.start_col - 1), "%[%[[^]#]*$")

  if check ~= nil then
    local kind = require("blink.cmp.types").CompletionItemKind.Reference

    util.try(function()
      return bin.get_thin_notes(config.current_vault.dir, config.current_vault.db)
    end, function(notes)
      for _, note in ipairs(notes) do
        items[#items + 1] = {
          label = note.title,
          filterText = note.title .. " " .. note.id,
          kind = kind,
          textEdit = {
            newText = "[[" .. note.id .. "$1|${2:" .. note.title .. "}]]$0",
            range = {
              start = {
                line = ctx.bounds.line_number - 1,
                character = ctx.bounds.start_col - check:len() - 1,
              },
              ["end"] = {
                line = ctx.bounds.line_number - 1,
                character = ctx.bounds.start_col + ctx.bounds.length + 1,
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
