# Fixture tests for automatic issue triage rules and playtest/triage.ps1.
# Covers: non-AI shortcuts, invalid JSON, out-of-range priority, preserving
# `em-andamento`, skipping closed/already-triaged issues, and -DryRun without writes.
# Run: pwsh -NoProfile -File ./playtest/test-triage.ps1

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "triage-rules.ps1")

$fixturesFile = Join-Path $PSScriptRoot "fixtures/triage/issues.json"
$issues = @(Get-Content $fixturesFile -Raw | ConvertFrom-Json)
$byNumber = @{}
foreach ($item in $issues) { $byNumber[[int]$item.number] = $item }

# 1. Unit tests for non-AI shortcuts (before calling AI).
$scCrash = Get-TriageShortcut $byNumber[710]
if ($scCrash.MinPriority -ne 5 -or $scCrash.FixedPriority -ne 5) {
    throw "shortcut #710 (kind crash): expected min/fixed 5, got min=$($scCrash.MinPriority) fixed=$($scCrash.FixedPriority)"
}

$scTravou = Get-TriageShortcut $byNumber[740]
if ($scTravou.MinPriority -ne 5 -or $scTravou.FixedPriority -ne 5) {
    throw "shortcut #740 (travou): expected min/fixed 5, got min=$($scTravou.MinPriority) fixed=$($scTravou.FixedPriority)"
}

$scIdeia = Get-TriageShortcut $byNumber[690]
if ($scIdeia.MaxPriority -ne 3) {
    throw "shortcut #690 (ideia without data loss): expected max 3, got $($scIdeia.MaxPriority)"
}

$scIdeiaLoss = Get-TriageShortcut $byNumber[691]
if ($scIdeiaLoss.MaxPriority -ne 5) {
    throw "shortcut #691 (ideia with data loss): expected max 5, got $($scIdeiaLoss.MaxPriority)"
}

$scNameless = Get-TriageShortcut $byNumber[801]
if ($scNameless.MinPriority -ne 2 -or $scNameless.MaxPriority -ne 2 -or $scNameless.FixedPriority -ne 2) {
    throw "shortcut #801 (explore nameless): expected fixed 2, got min=$($scNameless.MinPriority) max=$($scNameless.MaxPriority)"
}

$altNameless = [pscustomobject]@{
    number = 802
    title = "[explore] elemento sem nome acessível em 'Ajustes'"
    body = ""
    state = "OPEN"
    labels = @("explore")
}
if ((Get-TriageShortcut $altNameless).FixedPriority -ne 2) {
    throw "shortcut #802 (exact phrase 'elemento sem nome acessível'): expected fixed 2"
}

# 2. Unit tests for JSON validation and out-of-range priorities.
$badJson = ConvertFrom-TriageModelOutput "texto livre sem json" 905
if ($badJson.Valid) { throw "invalid JSON must be rejected" }

$truncatedJson = ConvertFrom-TriageModelOutput '{"priority":3,"area":' 905
if ($truncatedJson.Valid) { throw "truncated JSON must be rejected" }

foreach ($badPriority in @(0, 6, -1, 3.5, '"3"', 'true')) {
    $raw = "{`"priority`":$badPriority,`"area`":`"area-ajustes`",`"rationale`":`"Teste.`",`"duplicate_of`":null}"
    $res = ConvertFrom-TriageModelOutput $raw 906
    if ($res.Valid) { throw "priority '$badPriority' must be rejected" }
}

$badArea = ConvertFrom-TriageModelOutput '{"priority":3,"area":"area-fantasma","rationale":"Teste.","duplicate_of":null}' 906
if ($badArea.Valid) { throw "unknown area must be rejected" }

$fencedValid = ConvertFrom-TriageModelOutput "``````json`n{`"priority`":4,`"area`":`"area-editor`",`"rationale`":`"Falha no salvamento.`",`"duplicate_of`":null}`n``````" 901
if (-not $fencedValid.Valid -or $fencedValid.Priority -ne 4 -or $fencedValid.Area -ne "area-editor") {
    throw "fenced valid JSON should parse cleanly"
}

# 3. Eligibility checks: closed and already-triaged issues.
if (Test-TriageEligibleIssue $byNumber[903] -Retriage) {
    throw "closed issue #903 must never be eligible even with -Retriage"
}
if (Test-TriageEligibleIssue $byNumber[904]) {
    throw "already-triaged #904 must be skipped when -Retriage is false"
}
if (-not (Test-TriageEligibleIssue $byNumber[904] -Retriage)) {
    throw "already-triaged #904 should be eligible when -Retriage is true"
}
if (-not (Test-TriageEligibleIssue $byNumber[901])) {
    throw "open issue #901 with em-andamento and no priority must be eligible"
}

# 4. End-to-end tests with fake gh + fake opencode.
$fakeDir = Join-Path ([System.IO.Path]::GetTempPath()) "forja-triage-test-$PID"
$oldPath = $env:PATH
$oldFixtures = $env:FORJA_TRIAGE_FIXTURES
$oldWrites = $env:FORJA_FAKE_GH_WRITES
$oldOpencode = $env:FORJA_OPENCODE_SCRIPT

try {
    $null = New-Item -ItemType Directory -Force $fakeDir
    Copy-Item (Join-Path $PSScriptRoot "fixtures/triage/fake-gh.ps1") $fakeDir
    Set-Content -Path (Join-Path $fakeDir "gh.cmd") -Encoding ascii -Value @"
@echo off
pwsh -NoProfile -File "%~dp0fake-gh.ps1" %*
"@
    $env:PATH = "$fakeDir;$oldPath"
    $env:FORJA_TRIAGE_FIXTURES = $fixturesFile
    $env:FORJA_OPENCODE_SCRIPT = (Join-Path $PSScriptRoot "fixtures/triage/fake-opencode.ps1")

    # 4a. -DryRun must succeed with zero writes (FORJA_FAKE_GH_WRITES unset).
    Remove-Item Env:FORJA_FAKE_GH_WRITES -ErrorAction SilentlyContinue
    $dryRunOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -MaxIssues 30 -DryRun `
        -Models "mock/rate-limited-free,mock/primary-free" *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "DryRun failed with exit code $LASTEXITCODE`n$dryRunOut" }
    if ($dryRunOut -notmatch "DRY RUN #710: add \[prioridade-5, area-explorador\]") {
        throw "DryRun #710 did not enforce crash shortcut priority 5:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #740: add \[prioridade-5, area-terminal\]") {
        throw "DryRun #740 did not enforce travou shortcut priority 5:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #690: add \[prioridade-3, area-uso\]") {
        throw "DryRun #690 did not enforce ideia max 3:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #691: add \[prioridade-5, area-editor\]") {
        throw "DryRun #691 did not allow priority 5 for ideia with data loss:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #801: add \[prioridade-2, area-bigorna\]") {
        throw "DryRun #801 did not enforce explore nameless shortcut priority 2:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #901:.*preserving em-andamento.*Triagem: prioridade 4 \(área area-editor\)") {
        throw "DryRun #901 did not preserve em-andamento or format one-line comment:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #907: add \[prioridade-4, area-chat\]") {
        throw "DryRun #907 did not fall back to second model:`n$dryRunOut"
    }
    if ($dryRunOut -match "DRY RUN #903" -or $dryRunOut -match "DRY RUN #904" -or
        $dryRunOut -match "DRY RUN #905" -or $dryRunOut -match "DRY RUN #906") {
        throw "DryRun included closed/already-triaged/invalid issues:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "ignoring invalid model output") {
        throw "DryRun did not log ignored invalid/out-of-range outputs:`n$dryRunOut"
    }

    # 4b. Live-write mode against fake-gh: verify exact writes, preserved em-andamento,
    # invalid JSON/out-of-range produce zero writes, and closed #903 is never touched.
    $writeLog = Join-Path $fakeDir "writes.txt"
    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $env:FORJA_FAKE_GH_WRITES = $writeLog

    $applyOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -MaxIssues 30 `
        -Models @("mock/primary-free") *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "Apply run failed:`n$applyOut" }
    $writes = @(Get-Content $writeLog)
    if ($writes -match "903" -or $writes -match "904" -or $writes -match "905" -or $writes -match "906") {
        throw "Unexpected writes for skipped/invalid issues:`n$($writes -join "`n")"
    }
    if ($writes -match "--remove-label em-andamento" -or $writes -match "label create em-andamento") {
        throw "Triager touched protected label em-andamento:`n$($writes -join "`n")"
    }
    $comment901 = $writes | Where-Object { $_ -match "^issue comment 901 " } | Select-Object -First 1
    if ($comment901 -notmatch "Triagem: prioridade 4 \(área area-editor\) — ") {
        throw "Missing expected single-line comment on #901:`n$($writes -join "`n")"
    }

    # 4c. Closed #903 explicitly targeted with -IssueNumber -Retriage must produce 0 writes.
    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $closedOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -IssueNumber 903 -Retriage -Models @("mock/primary-free") *>&1 | Out-String
    if ($closedOut -notmatch "skip #903: issue is closed") {
        throw "Closed issue #903 was not skipped cleanly:`n$closedOut"
    }
    if (Test-Path $writeLog) {
        throw "Closed issue #903 triggered writes: $(Get-Content $writeLog -Raw)"
    }

    # 4d. -Retriage on #902 replaces prioridade-2/area-layout, adds duplicata?, preserves em-andamento.
    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $retriageOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -IssueNumber 902 -Retriage -Models @("mock/primary-free") *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "Retriage #902 failed:`n$retriageOut" }
    $retriageWrites = @(Get-Content $writeLog)
    $edit902 = $retriageWrites | Where-Object { $_ -match "^issue edit 902 " } | Select-Object -First 1
    if (-not $edit902) { throw "Retriage #902 did not edit labels:`n$($retriageWrites -join "`n")" }
    if ($edit902 -notmatch "--add-label prioridade-4" -or $edit902 -notmatch "--add-label area-editor" -or $edit902 -notmatch "--add-label duplicata\?") {
        throw "Retriage #902 missing expected added labels: $edit902"
    }
    if ($edit902 -notmatch "--remove-label prioridade-2" -or $edit902 -notmatch "--remove-label area-layout") {
        throw "Retriage #902 did not remove stale priority/area labels: $edit902"
    }
    if ($edit902 -match "em-andamento") {
        throw "Retriage #902 must NEVER touch em-andamento: $edit902"
    }
} finally {
    $env:PATH = $oldPath
    if ($null -eq $oldFixtures) { Remove-Item Env:FORJA_TRIAGE_FIXTURES -ErrorAction SilentlyContinue } else { $env:FORJA_TRIAGE_FIXTURES = $oldFixtures }
    if ($null -eq $oldWrites) { Remove-Item Env:FORJA_FAKE_GH_WRITES -ErrorAction SilentlyContinue } else { $env:FORJA_FAKE_GH_WRITES = $oldWrites }
    if ($null -eq $oldOpencode) { Remove-Item Env:FORJA_OPENCODE_SCRIPT -ErrorAction SilentlyContinue } else { $env:FORJA_OPENCODE_SCRIPT = $oldOpencode }
    Remove-Item $fakeDir -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "triage fixtures: all shortcut, validation, em-andamento preservation, retriage, fallback, and DryRun tests passed"
