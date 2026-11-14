<##
.SYNOPSIS
  Seleciona issues abertas que aguardam verificação de uma correção.
#>
param(
    [int]$MaxIssues = 8,
    [string]$Repository = $env:GH_REPO,
    [string]$Out = "verification-issues.json"
)

$ErrorActionPreference = "Stop"
if ($MaxIssues -lt 1) { throw "MaxIssues must be at least 1" }
if (-not $Repository) { throw "Repository is required (OWNER/REPO)" }

$owner = (& gh api "repos/$Repository" --jq ".owner.login").Trim()
if ($LASTEXITCODE -ne 0 -or -not $owner) { throw "could not read repository owner" }

$openIssues = @(& gh issue list -R $Repository --state open --limit 1000 `
    --json number,title,createdAt,labels,url | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw "could not list open issues" }

$selected = [System.Collections.Generic.List[object]]::new()
$eligibleLabels = @("playtest", "explore", "ideia")
foreach ($listed in ($openIssues | Sort-Object { [DateTimeOffset]$_.createdAt })) {
    $labels = @($listed.labels | ForEach-Object { $_.name })
    if (-not ($labels | Where-Object { $_ -in $eligibleLabels })) { continue }

    $detail = & gh issue view $listed.number -R $Repository `
        --json number,title,body,state,labels,comments,url | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "could not read issue #$($listed.number)" }
    if ($detail.state -ne "OPEN") { continue }

    $comments = @($detail.comments | Sort-Object { [DateTimeOffset]$_.createdAt })
    $correction = $comments |
        Where-Object {
            $_.author.login -eq $owner -and
            ($_.body.StartsWith("Corrigido na main") -or
             $_.body.StartsWith("Tratado na main") -or
             $_.body.StartsWith("Concluído na main"))
        } |
        Select-Object -Last 1
    if (-not $correction) { continue }

    $lastComment = $comments | Select-Object -Last 1
    if ($lastComment -and $lastComment.body.StartsWith("Verificado:") -and
        [DateTimeOffset]$lastComment.createdAt -ge [DateTimeOffset]$correction.createdAt) {
        continue
    }

    $selected.Add([pscustomobject]@{
        issue = [int]$detail.number
        title = $detail.title
        body = $detail.body
        url = $detail.url
        labels = $labels
        correction = [pscustomobject]@{
            id = $correction.id
            body = $correction.body
            createdAt = $correction.createdAt
            url = $correction.url
        }
    })
    if ($selected.Count -ge $MaxIssues) { break }
}

$outPath = [System.IO.Path]::GetFullPath($Out)
$parent = Split-Path -Parent $outPath
if ($parent) { New-Item -ItemType Directory -Force $parent | Out-Null }
ConvertTo-Json -InputObject @($selected) -Depth 20 | Set-Content $outPath -Encoding utf8
Write-Host "verification: selected $($selected.Count) open issue(s) oldest first"
if ($env:GITHUB_OUTPUT) {
    $hasIssues = if ($selected.Count -gt 0) { "true" } else { "false" }
    "has_issues=$hasIssues" | Add-Content $env:GITHUB_OUTPUT
    "issue_count=$($selected.Count)" | Add-Content $env:GITHUB_OUTPUT
}
