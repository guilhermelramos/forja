-- Conta TODO e FIXME nos arquivos do projeto.

forja.settings.schema({
  {
    name = "max_files",
    type = "number",
    default = 300,
    range = { min = 10, max = 2000 },
    label = { ["pt-BR"] = "Arquivos analisados", en = "Files scanned" },
    help = { ["pt-BR"] = "Limite de arquivos lidos por contagem.", en = "File limit per scan." },
  },
})

local MARKERS = { "TODO", "FIXME" }
local PATTERNS = { "**/*.rs", "**/*.ts", "**/*.md" }

local function count()
  local maximum = forja.settings.get("max_files")
  local rows, read = {}, 0
  for _, pattern in ipairs(PATTERNS) do
    local ok_list, paths = pcall(forja.fs.list, pattern)
    for _, path in ipairs(ok_list and paths or {}) do
      if read >= maximum then break end
      read = read + 1
      local ok, text = pcall(forja.fs.read, path)
      if ok then
        local total = 0
        for _, marker in ipairs(MARKERS) do
          local _, n = text:gsub(marker, "")
          total = total + n
        end
        if total > 0 then
          rows[#rows + 1] = { id = path, file = path, total = total }
        end
      end
    end
  end
  table.sort(rows, function(a, b) return a.total > b.total end)
  return rows
end

local function refresh()
  local rows = count()
  local sum = 0
  for _, r in ipairs(rows) do sum = sum + r.total end
  forja.ui.panel({
    id = "todos",
    title = { ["pt-BR"] = "TODOs", en = "TODOs" },
    slot = "right_panel",
    tree = {
      forja.ui.text({ value = ("%d ocorrências em %d arquivos"):format(sum, #rows), style = "title" }),
      forja.ui.button({
        id = "refresh",
        label = { ["pt-BR"] = "Atualizar", en = "Refresh" },
        action = function(ev) refresh() end,
      }),
      forja.ui.table({
        id = "table",
        columns = {
          { key = "file", label = { ["pt-BR"] = "Arquivo", en = "File" } },
          { key = "total", label = "TODOs", width = 80 },
        },
        rows = rows,
      }),
    },
  })
end

forja.commands.register({
  id = "count",
  title = { ["pt-BR"] = "Contar TODOs no projeto", en = "Count TODOs in project" },
  run = function(args, ctx)
    refresh()
    return { message = "Contagem atualizada." }
  end,
})

forja.on("project.opened", function(ev)
  refresh()
end)

refresh()
