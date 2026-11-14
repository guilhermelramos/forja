<#
.SYNOPSIS
  Junta os achados dos testadores, abre issues novas (sem repetir) e escreve o relatório.

.DESCRIPTION
  Lê <In>/<missão>/findings.jsonl. Cada achado novo vira uma issue com o
  rótulo `playtest`. Um achado repetido (mesmo título normalizado, ou títulos
  quase iguais) de uma issue aberta ou fechada não gera outra. No máximo
  -MaxIssues por rodada. Escreve report.md com tudo (para o resumo do job).
#>
param(
    [string]$In = "playtest-results",
    [int]$MaxIssues = 10,
    [string]$Report = "report.md",
    # Issue label and title prefix (playtest for bugs, ideia for the ideas loop).
    [string]$Label = "playtest",
    [string[]]$Labels = @(),
    [string]$TitlePrefix = "",
    [string]$SourceNote = "Registrado por um testador automático (modelo gratuito via OpenCode) usando a Forja ao vivo.",
    [switch]$DryRun
)

if ($Labels.Count -eq 0) { $Labels = @($Label) }
if (-not $TitlePrefix) { $TitlePrefix = "[$($Labels[0])]" }

$ErrorActionPreference = "Stop"

function Normalize([string]$text) {
    ($text.ToLowerInvariant() -replace "[^\p{L}\p{N} ]", " " -replace "\s+", " ").Trim()
}
function Similar([string]$a, [string]$b) {
    $wa = @((Normalize $a) -split " " | Where-Object { $_.Length -gt 2 } | Select-Object -Unique)
    $wb = @((Normalize $b) -split " " | Where-Object { $_.Length -gt 2 } | Select-Object -Unique)
    if ($wa.Count -eq 0 -or $wb.Count -eq 0) { return $false }
    $common = @($wa | Where-Object { $wb -contains $_ }).Count
    return ($common / [Math]::Max($wa.Count, $wb.Count)) -ge 0.6
}

$all = @()
foreach ($dir in Get-ChildItem $In -Directory -ErrorAction SilentlyContinue) {
    $file = Get-ChildItem $dir.FullName -Recurse -Filter findings.jsonl | Select-Object -First 1
    if (-not $file) { continue }
    foreach ($line in Get-Content $file.FullName) {
        if (-not $line.Trim()) { continue }
        $finding = $line | ConvertFrom-Json
        # A clean exit (code 0) is the tester closing the window, not a crash.
        if ($finding.kind -eq "crash" -and [string]$finding.actual -match "código 0\.") { continue }
        $finding | Add-Member -NotePropertyName mission -NotePropertyValue ($dir.Name -replace '^(playtest|explore)-', '') -Force
        $all += $finding
    }
}

$existing = @()
if (-not $DryRun) {
    foreach ($lbl in $Labels) {
        gh label create $lbl --color D93F0B --description "Achado pelos testadores automáticos" --force 2>$null | Out-Null
        $existing += @(gh issue list --label $lbl --state all --limit 500 --json title | ConvertFrom-Json | ForEach-Object { $_.title -replace '^\[[^\]]+\]\s*', '' })
    }
    $existing = @($existing | Select-Object -Unique)
}

$order = @{ critical = 0; high = 1; medium = 2; low = 3 }
$created = 0
$lines = @("# Rodada de testes", "", "$($all.Count) achados de $(@($all | Select-Object -ExpandProperty mission -Unique).Count) testadores.", "")
foreach ($finding in $all | Sort-Object { $order[[string]$_.severity] ?? 4 }) {
    $title = [string]$finding.title
    $duplicate = $existing | Where-Object { (Normalize $_) -eq (Normalize $title) -or (Similar $_ $title) } | Select-Object -First 1
    $status = "nova"
    if ($duplicate) {
        $status = "já existe: $duplicate"
    } elseif ($created -ge $MaxIssues) {
        $status = "não aberta (limite de $MaxIssues por rodada)"
    } else {
        $steps = @($finding.steps | Where-Object { $_ }) | ForEach-Object { "1. $_" }
        $body = @(
            "**Missão:** $($finding.mission) · **Tipo:** $($finding.kind) · **Gravidade:** $($finding.severity)",
            "**Tela:** $($finding.screen)", "",
            "### Passos", ($steps -join "`n"), "",
            "### Esperado", [string]$finding.expected, "",
            "### Aconteceu", [string]$finding.actual, "",
            "_${SourceNote} Rodada: $env:GITHUB_SERVER_URL/$env:GITHUB_REPOSITORY/actions/runs/$env:GITHUB_RUN_ID_"
        ) -join "`n"
        if (-not $DryRun) {
            $body | Set-Content issue-body.md -Encoding utf8
            $labelArgs = @()
            foreach ($lbl in $Labels) { $labelArgs += @("--label", $lbl) }
            gh issue create --title "$TitlePrefix $title" @labelArgs --body-file issue-body.md | Out-Null
        }
        $existing += $title
        $created++
    }
    $lines += "- **[$($finding.severity)]** $title — _$($finding.mission)_ ($status)"
}
$lines += "", "$created issues novas."
$usageFiles = @(Get-ChildItem $In -Directory -ErrorAction SilentlyContinue | ForEach-Object {
    $file = Get-ChildItem $_.FullName -Recurse -Filter usage.json | Select-Object -First 1
    if ($file) { [pscustomobject]@{ Dir = $_; File = $file } }
})
if ($usageFiles.Count -gt 0) {
    $lines += "", "## Uso de tokens", "",
        "| Testador | Passos | Entrada | Cache lido | Saída | Raciocínio | Custo |", "|---|---:|---:|---:|---:|---:|---:|"
    $sum = @{ steps = [long]0; input = [long]0; cache = [long]0; output = [long]0; reasoning = [long]0; cost = 0.0 }
    foreach ($entry in $usageFiles) {
        try { $usage = Get-Content $entry.File.FullName -Raw | ConvertFrom-Json } catch { continue }
        $row = @{
            steps = [long]$usage.steps; input = [long]$usage.tokens.input; cache = [long]$usage.tokens.cache.read
            output = [long]$usage.tokens.output; reasoning = [long]$usage.tokens.reasoning; cost = [double]$usage.cost
        }
        foreach ($key in @($row.Keys)) { $sum[$key] += $row[$key] }
        $lines += "| $($entry.Dir.Name -replace '^(playtest|explore)-', '') | $($row.steps) | $($row.input.ToString('N0')) | $($row.cache.ToString('N0')) | $($row.output.ToString('N0')) | $($row.reasoning.ToString('N0')) | US$ $($row.cost.ToString('N4')) |"
    }
    $lines += "| **total** | $($sum.steps) | $($sum.input.ToString('N0')) | $($sum.cache.ToString('N0')) | $($sum.output.ToString('N0')) | $($sum.reasoning.ToString('N0')) | US$ $($sum.cost.ToString('N4')) |"
}
$lines -join "`n" | Set-Content $Report -Encoding utf8
Write-Host ($lines -join "`n")
