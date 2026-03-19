local M = {}

M.get_paths = function ()
    local source = debug.getinfo(1, "S").source:sub(2)
    local plugin_root = vim.fn.fnamemodify(source, ":h:h:h")
    local data_dir = vim.fn.stdpath("data") .. "/markdown_db"

    return {
        plugin_root = plugin_root,
        data_dir = data_dir,
        release_dir = plugin_root .. "/target/release",
    }
end

return M
