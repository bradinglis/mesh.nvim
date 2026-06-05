local bin = require "mesh-nvim.binary"
local config = require "mesh-nvim.config"
local util = require "mesh-nvim.util"
local source = {}

function source.new(opts)
  local self = setmetatable({}, { __index = source })
  self.opts = opts
  return self
end

function source:get_trigger_characters() return { '#' } end

function source:enabled() return vim.bo.filetype == 'markdown' end

function source:get_completions(ctx, callback)
  --- @type lsp.CompletionItem[]
  local items = {}

  local check = string.match(string.sub(ctx.line, 0, ctx.bounds.start_col-1), "([^ ]*)#[^ ]*$")

  if ctx.line:sub(1, 7) == "author:" or ctx.line:sub(1, 15) == "source-parents:" or ctx.line:sub(1, 11) == "references:" then
    if check then
      util.try(function()
          return bin.get_note_anchors(config.current_vault.dir, config.current_vault.db, check)
        end,
        function(anchors)
          for _, anchor in ipairs(anchors) do
            local item = {
              label = anchor.title,
              filterText = anchor.title .. " " .. anchor.id,
              kind = require('blink.cmp.types').CompletionItemKind.Reference,
              textEdit = {
                newText =  anchor.id ,
                range = {
                  start = { line = ctx.bounds.line_number - 1, character = ctx.bounds.start_col - check:len() - 2 },
                  ['end'] = { line = ctx.bounds.line_number - 1, character = ctx.bounds.start_col + ctx.bounds.length - 1 },
                },
              },
              insertTextFormat = vim.lsp.protocol.InsertTextFormat.PlainText,
            }
            table.insert(items, item)
          end
        end)
    else
      util.try(function()
          return bin.get_thin_notes(config.current_vault.dir, config.current_vault.db)
        end,
        function(notes)
          for _, note in ipairs(notes) do
            local item = {
              label = note.id .. " (" .. note.title .. ")",
              filterText = note.id .. " " .. note.title,
              kind = require('blink.cmp.types').CompletionItemKind.Reference,
              textEdit = {
                newText = note.id,
                range = {
                  start = { line = ctx.bounds.line_number - 1, character = ctx.bounds.start_col - 1 },
                  ['end'] = { line = ctx.bounds.line_number - 1, character = ctx.bounds.start_col + ctx.bounds.length - 1 },
                },
              },
              insertTextFormat = vim.lsp.protocol.InsertTextFormat.PlainText,
            }
            table.insert(items, item)
          end
        end)
    end
  end
  -- if check ~= nil then
  --   util.try(function()
  --       return bin.get_thin_notes(config.current_vault.dir, config.current_vault.db)
  --     end,
  -- end
  --
  callback({
    items = items,
    is_incomplete_backward = true,
    is_incomplete_forward = true,
  })

  return function() end
end

-- (Optional) Before accepting the item or showing documentation, blink.cmp will call this function
-- so you may avoid calculating expensive fields (i.e. documentation) for only when they're actually needed
-- Note only some fields may be resolved lazily. You may check the LSP capabilities for a complete list:
-- `textDocument.completion.completionItem.resolveSupport`
-- At the time of writing: 'documentation', 'detail', 'additionalTextEdits', 'command', 'data'
-- function source:resolve(item, callback)
--   item = vim.deepcopy(item)
--
--   -- Shown in the documentation window (<C-space> when menu open by default)
--   item.documentation = {
--     kind = 'markdown',
--     value = '# Foo\n\nBar',
--   }
--
--   -- Additional edits to make to the document, such as for auto-imports
--   item.additionalTextEdits = {
--     {
--       newText = 'foo',
--       range = {
--         start = { line = 0, character = 0 },
--         ['end'] = { line = 0, character = 0 },
--       },
--     },
--   }
--
--   callback(item)
-- end

function source:execute(ctx, item, callback, default_implementation)
  default_implementation()
  callback()
end

return source
