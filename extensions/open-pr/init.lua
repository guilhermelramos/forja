-- Página de PR da branch atual.

forja.settings.schema({
  {
    name = "base",
    type = "text",
    default = "main",
    label = { ["pt-BR"] = "Branch base do PR", en = "PR base branch" },
  },
})

-- Aceita git@github.com:owner/repo.git e https://github.com/owner/repo(.git).
local function owner_and_repo(remote)
  local owner, repo = remote:match("github%.com[:/]([^/]+)/([^/]+)$")
  if not owner then return nil end
  return owner, (repo:gsub("%.git$", ""))
end

local function pr_link(branch, base)
  local remote = forja.git.remote_url("origin")
  if not remote then return nil, "a bancada não tem remoto origin" end
  local owner, repo = owner_and_repo(remote)
  if not owner then return nil, "o remoto origin não é do GitHub" end
  return ("https://github.com/%s/%s/compare/%s...%s?expand=1"):format(owner, repo, base, branch)
end

forja.commands.register({
  id = "open",
  title = { ["pt-BR"] = "Abrir PR da branch atual", en = "Open PR for current branch" },
  run = function(args, ctx)
    local branch = forja.git.branch()
    if branch == "" or branch == "HEAD" then
      forja.notify({ title = "Abrir PR", text = "A bancada não está numa branch.", level = "warning" })
      return nil
    end
    local url, err = pr_link(branch, forja.settings.get("base"))
    if not url then
      forja.notify({ title = "Abrir PR", text = err, level = "error" })
      return nil
    end
    -- Permitido porque o usuário rodou o comando pela Bigorna.
    forja.ui.open_url(url)
    return { message = "Página de PR aberta." }
  end,
})
