<##
.SYNOPSIS
  Seleciona issues que aguardam verificação de uma correção.
#>
param(
    [int]$MaxIssues = 8,
    [string]$Repository = $env:GH_REPO,
    [string]$Out = "verification-issues.json"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "verdict-rules.ps1")
if ($MaxIssues -lt 1) { throw "MaxIssues must be at least 1" }
if (-not $Repository) { throw "Repository is required (OWNER/REPO)" }

$owner = (& gh api "repos/$Repository" --jq ".owner.login").Trim()
if ($LASTEXITCODE -ne 0 -or -not $owner) { throw "could not read repository owner" }

function Get-ListedIssues([string]$State, [string]$Search) {
    $arguments = @(
        "issue", "list", "-R", $Repository,
        "--state", $State, "--limit", "1000",
        "--json", "number,title,createdAt,labels,url"
    )
    if ($Search) { $arguments += @("--search", $Search) }
    $json = & gh @arguments
    if ($LASTEXITCODE -ne 0) { throw "could not list $State issues" }
    return @($json | ConvertFrom-Json)
}

# Open issues are the normal case. Closed issues are searched by last update so
# a correction posted after the close (a re-check request) is found even in an
# old issue, without paging the whole repository history.
$listedByNumber = @{}
$listed = @(Get-ListedIssues "open" "sort:updated-desc") +
    @(Get-ListedIssues "closed" "sort:updated-desc")
foreach ($item in $listed) {
    if ($null -eq $item) { continue }
    $listedByNumber[[int]$item.number] = $item
}

$selected = [System.Collections.Generic.List[object]]::new()
$eligibleLabels = @("playtest", "explore", "ideia")
$inProgressLabel = "em-andamento"
$priorityPattern = '^prioridade-([1-5])$'

function Get-IssuePriorityRank($Issue) {
    $rank = 0
    foreach ($label in @($Issue.labels)) {
        if ($null -eq $label) { continue }
        $name = if ($label -is [string]) { $label } else { [string]$label.name }
        if ($name -match $priorityPattern) {
            $value = [int]$Matches[1]
            if ($value -gt $rank) { $rank = $value }
        }
    }
    return $rank
}

# Triaged issues are verified highest priority first (prioridade-5 down to
# prioridade-1, then issues without a priority label); within the same priority,
# the oldest issue comes first. The number only makes ties deterministic.
$ordered = @(
    $listedByNumber.Values |
        Where-Object { $null -ne $_ } |
        ForEach-Object {
            [pscustomobject]@{
                Issue = $_
                Number = [int]$_.number
                Priority = Get-IssuePriorityRank $_
                CreatedAt = [DateTimeOffset]$_.createdAt
            }
        } |
        Sort-Object -Property `
            @{ Expression = 'Priority'; Descending = $true },
            @{ Expression = 'CreatedAt'; Descending = $false },
            @{ Expression = 'Number'; Descending = $false }
)
foreach ($entry in $ordered) {
    $listedIssue = $entry.Issue
    $labels = @($listedIssue.labels | ForEach-Object {
        if ($_ -is [string]) { $_ } else { $_.name }
    })
    if (-not ($labels | Where-Object { $_ -in $eligibleLabels })) { continue }
    if ($labels -contains $inProgressLabel) {
        Write-Host "skip #$($entry.Number): label em-andamento, a worker is still changing it; not verifying"
        continue
    }

    $detail = & gh issue view $listedIssue.number -R $Repository `
        --json number,title,body,state,labels,comments,url,closedAt | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "could not read issue #$($listedIssue.number)" }

    $candidate = Get-PendingCorrection $detail $owner
    if ($null -eq $candidate) { continue }

    if ($detail.state -eq 'CLOSED') {
        if ($null -eq $candidate.lastVerdict -or -not $detail.closedAt -or
            [DateTimeOffset]$candidate.correction.createdAt -le [DateTimeOffset]$detail.closedAt) {
            continue
        }
        $closeActor = Get-IssueCloseActor $Repository ([int]$detail.number)
        if ($closeActor -ne $script:BotCloseActor) {
            $who = if ($closeActor) { $closeActor } else { 'unknown' }
            Write-Host "skip #$($detail.number): closed by $who, not github-actions[bot]; not re-checking"
            continue
        }
        $detail | Add-Member -NotePropertyName closeActor -NotePropertyValue $closeActor -Force
    }

    $pending = Get-PendingVerification $detail $owner
    if ($null -eq $pending) { continue }

    $selected.Add([pscustomobject]@{
        issue = [int]$detail.number
        title = $detail.title
        body = $detail.body
        url = $detail.url
        labels = $labels
        priority = [int]$entry.Priority
        state = $detail.state
        recheck = [bool]$pending.recheck
        correction = [pscustomobject]@{
            id = $pending.correction.id
            body = $pending.correction.body
            createdAt = $pending.correction.createdAt
            url = $pending.correction.url
        }
    })
    if ($selected.Count -ge $MaxIssues) { break }
}

$outPath = [System.IO.Path]::GetFullPath($Out)
$parent = Split-Path -Parent $outPath
if ($parent) { New-Item -ItemType Directory -Force $parent | Out-Null }
ConvertTo-Json -InputObject @($selected) -Depth 20 | Set-Content $outPath -Encoding utf8
$orderText = if ($selected.Count -gt 0) {
    ($selected | ForEach-Object {
        $priorityText = if ($_.priority -gt 0) { "prioridade-$($_.priority)" } else { "sem prioridade" }
        "#$($_.issue) ($priorityText)"
    }) -join ', '
} else {
    "none"
}
Write-Host "verification: selected $($selected.Count) issue(s) awaiting verification, highest priority first and oldest first within the same priority: $orderText"
if ($env:GITHUB_OUTPUT) {
    $hasIssues = if ($selected.Count -gt 0) { "true" } else { "false" }
    "has_issues=$hasIssues" | Add-Content $env:GITHUB_OUTPUT
    "issue_count=$($selected.Count)" | Add-Content $env:GITHUB_OUTPUT
}
