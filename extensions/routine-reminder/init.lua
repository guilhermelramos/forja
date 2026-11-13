-- Aviso e último resultado das rotinas vigiadas.

forja.settings.schema({
  {
    name = "routines",
    type = "text",
    default = "",
    label = { ["pt-BR"] = "Rotinas vigiadas", en = "Watched routines" },
    help = { ["pt-BR"] = "Nomes separados por vírgula.", en = "Comma-separated names." },
  },
  {
    name = "notify_success",
    type = "toggle",
    default = false,
    label = { ["pt-BR"] = "Avisar quando dá certo", en = "Notify on success" },
  },
})

local function watched()
  local names = {}
  for name in forja.settings.get("routines"):gmatch("[^,]+") do
    names[#names + 1] = name:match("^%s*(.-)%s*$")
  end
  return names
end

local function is_watched(name)
  for _, v in ipairs(watched()) do
    if v == name then return true end
  end
  return false
end

forja.on("routine.finished", function(ev)
  if not is_watched(ev.name) then return end
  forja.store.set("last:" .. ev.name, { summary = ev.summary, at = forja.time.now(), ok = true })
  if forja.settings.get("notify_success") then
    forja.notify({ title = forja.i18n.t("routine_done"), text = ev.name .. ": " .. ev.summary, level = "info" })
  end
end)

forja.on("routine.failed", function(ev)
  if not is_watched(ev.name) then return end
  forja.store.set("last:" .. ev.name, { summary = ev.error, at = forja.time.now(), ok = false })
  forja.notify({ title = forja.i18n.t("routine_failed"), text = ev.name .. ": " .. ev.error, level = "error" })
end)

forja.commands.register({
  id = "latest",
  title = { ["pt-BR"] = "Ver últimas rotinas vigiadas", en = "Show watched routines" },
  run = function(args, ctx)
    local rows = {}
    for _, name in ipairs(watched()) do
      local last = forja.store.get("last:" .. name)
      rows[#rows + 1] = {
        id = name,
        routine = name,
        result = last and last.summary or forja.i18n.t("no_result"),
        when = last and last.at or "—",
      }
    end
    forja.ui.panel({
      id = "routines",
      title = { ["pt-BR"] = "Rotinas", en = "Routines" },
      slot = "right_panel",
      tree = {
        forja.ui.table({
          id = "table",
          columns = {
            { key = "routine", label = { ["pt-BR"] = "Rotina", en = "Routine" } },
            { key = "result", label = { ["pt-BR"] = "Último resultado", en = "Last result" } },
            { key = "when", label = { ["pt-BR"] = "Quando", en = "When" }, width = 180 },
          },
          rows = rows,
        }),
      },
    })
    return nil
  end,
})
