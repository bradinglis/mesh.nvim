local bin = require "mesh-nvim.binary"
local config = require "mesh-nvim.config"
local util = require "mesh-nvim.util"
local completion = require "mesh-nvim.completion"

local reference_fields = {
  ["author:"] = true,
  ["source-parents:"] = true,
  ["references:"] = true,
}

return completion.module(function(ctx, callback)
  --- @type lsp.CompletionItem[]
  local items = {}

  local check = string.match(string.sub(ctx.line, 0, ctx.bounds.start_col - 1), "([^ ]*)#[^ ]*$")
  local field = ctx.line:match("^([%w%-]+:)")
  local kind = require("blink.cmp.types").CompletionItemKind.Reference

  if not reference_fields[field] then
    return completion.respond(callback, items)
  end

  if check then
    util.try(function()
      return bin.get_note_anchors(config.current_vault.dir, config.current_vault.db, check)
    end, function(anchors)
      for _, anchor in ipairs(anchors) do
        items[#items + 1] = {
          label = anchor.title,
          filterText = anchor.title .. " " .. anchor.id,
          kind = kind,
          textEdit = {
            newText = anchor.id,
            range = {
              start = {
                line = ctx.bounds.line_number - 1,
                character = ctx.bounds.start_col - check:len() - 2,
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
  else
    util.try(function()
      return bin.get_thin_notes(config.current_vault.dir, config.current_vault.db)
    end, function(notes)
      for _, note in ipairs(notes) do
        items[#items + 1] = {
          label = note.id .. " (" .. note.title .. ")",
          filterText = note.id .. " " .. note.title,
          kind = kind,
          textEdit = {
            newText = note.id,
            range = {
              start = {
                line = ctx.bounds.line_number - 1,
                character = ctx.bounds.start_col - 1,
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
  end

  return completion.respond(callback, items)
end)
