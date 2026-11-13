-- Commits de hoje por autor.

forja.settings.schema({
  {
    name = "author",
    type = "text",
    default = "",
    label = { ["pt-BR"] = "Autor (vazio = todos)", en = "Author (empty = everyone)" },
  },
  {
    name = "limit",
    type = "number",
    default = 100,
    range = { min = 10, max = 500 },
    label = { ["pt-BR"] = "Commits lidos", en = "Commits read" },
  },
})

local function summarize()
  local today = forja.time.today()
  local author = forja.settings.get("author")
  local commits = forja.git.log({ limit = forja.settings.get("limit"), since = today })
  local total, by_author = 0, {}
  for _, c in ipairs(commits) do
    if author == "" or c.author == author then
      total = total + 1
      by_author[c.author] = (by_author[c.author] or 0) + 1
    end
  end
  local rows = {}
  for name, n in pairs(by_author) do
    rows[#rows + 1] = { id = name, author = name, commits = n }
  end
  table.sort(rows, function(a, b) return a.commits > b.commits end)
  forja.ui.panel({
    id = "summary",
    title = { ["pt-BR"] = "Resumo do dia", en = "Day summary" },
    slot = "right_panel",
    tree = {
      forja.ui.text({ value = ("%d commits desde %s"):format(total, today), style = "title" }),
      forja.ui.table({
        id = "authors",
        columns = {
          { key = "author", label = { ["pt-BR"] = "Autor", en = "Author" } },
          { key = "commits", label = "Commits", width = 90 },
        },
        rows = rows,
      }),
    },
  })
  return total
end

forja.commands.register({
  id = "today",
  title = { ["pt-BR"] = "Resumo do dia (git)", en = "Day summary (git)" },
  run = function(args, ctx)
    local total = summarize()
    forja.notify({ title = "Resumo do dia", text = total .. " commits hoje", level = "info" })
    return nil
  end,
})
