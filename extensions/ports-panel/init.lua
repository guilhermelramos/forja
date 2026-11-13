-- Portas abertas da bancada.

forja.settings.schema({
  {
    name = "ignore",
    type = "text",
    default = "",
    label = { ["pt-BR"] = "Portas ignoradas", en = "Ignored ports" },
    help = { ["pt-BR"] = "Números separados por vírgula, como 5432,6379.", en = "Comma-separated numbers, like 5432,6379." },
  },
})

local open = {}   -- porta (number) -> { process = texto }

local function ignored()
  local set = {}
  for number in forja.settings.get("ignore"):gmatch("%d+") do
    set[tonumber(number)] = true
  end
  return set
end

local function render()
  local ignore = ignored()
  local items = {}
  for port, info in pairs(open) do
    if not ignore[port] then
      items[#items + 1] = {
        id = tostring(port),
        label = ("localhost:%d"):format(port),
        detail = info.process or "",
      }
    end
  end
  table.sort(items, function(a, b) return tonumber(a.id) < tonumber(b.id) end)
  forja.ui.panel({
    id = "ports",
    title = { ["pt-BR"] = "Portas", en = "Ports" },
    slot = "right_panel",
    tree = {
      forja.ui.list({
        id = "list",
        items = items,
        action = function(ev)
          forja.ui.open_url("http://localhost:" .. ev.item)   -- clique; localhost não é bloqueado
        end,
      }),
      forja.ui.text({
        value = (#items == 0) and "Nenhuma porta aberta." or ("%d abertas"):format(#items),
        style = "secondary",
      }),
    },
  })
end

forja.on("port.detected", function(ev)
  open[ev.port] = { process = ev.process }
  render()
end)

forja.on("port.closed", function(ev)
  open[ev.port] = nil
  render()
end)

render()
