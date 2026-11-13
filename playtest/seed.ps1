<#
.SYNOPSIS
  Cria o repositório de exemplo que os testadores usam (C:\playground\exemplo).
#>
param([string]$Path = "C:\playground\exemplo")

$ErrorActionPreference = "Stop"
if (Test-Path $Path) { Remove-Item -Recurse -Force $Path }
New-Item -ItemType Directory -Force "$Path\src", "$Path\docs" | Out-Null
Set-Location $Path
git init -q -b main
git config user.name "Exemplo"
git config user.email "exemplo@example.invalid"

Set-Content README.md "# Exemplo`n`nProjeto de exemplo para os testes da Forja.`n" -Encoding utf8
Set-Content src\login.ts "export function login(user: string) {`n  return user.length > 0;`n}`n" -Encoding utf8
Set-Content src\app.py "def main():`n    print('ola')`n`nif __name__ == '__main__':`n    main()`n" -Encoding utf8
git add -A; git commit -q -m "Primeiro commit"

Set-Content docs\guia.md "# Guia`n`n- passo 1`n- passo 2`n" -Encoding utf8
git add -A; git commit -q -m "Guia inicial"

git switch -q -c feature/relatorio
Add-Content src\app.py "`ndef relatorio():`n    return 'mensal'`n"
git commit -qam "Relatório mensal"
git switch -q main

# Uma mudança sem commit, para as abas Mudanças e Diferenças terem conteúdo.
Add-Content src\login.ts "export const retry = 2;`n"
Write-Host "seed: $Path ($(git rev-list --count --all) commits, branches: $((git branch --format '%(refname:short)') -join ', '))"
