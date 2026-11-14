# Fixture test for issue selection and the end-to-end -DryRun flow.
# It uses the real history of #13 (closed by a person) and a bot-closed variant
# of that history. Only the bot-closed issue may be re-checked or reopened.
# Run: ./playtest/test-selection.ps1

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "verdict-rules.ps1")

$owner = "guilhermelramos"
$issuesDir = Join-Path $PSScriptRoot "fixtures/selection/issues"
$issue13 = Get-Content (Join-Path $issuesDir "issue-13.json") -Raw | ConvertFrom-Json
$issue1301 = Get-Content (Join-Path $issuesDir "issue-1301.json") -Raw | ConvertFrom-Json
$issue13Chatter = Get-Content (Join-Path $issuesDir "chatter-13.json") -Raw | ConvertFrom-Json
$issue64 = Get-Content (Join-Path $issuesDir "issue-64.json") -Raw | ConvertFrom-Json
$issue9 = Get-Content (Join-Path $issuesDir "issue-9.json") -Raw | ConvertFrom-Json
$issue3 = Get-Content (Join-Path $issuesDir "issue-3.json") -Raw | ConvertFrom-Json

# The real #13 was closed by a person after the bot had closed it once.
$close13 = Get-LastCloseActor @($issue13.events)
if ($close13 -ne "guilhermelramos") {
    throw "#13: expected the last close by guilhermelramos, got '$close13'"
}
$issue13 | Add-Member -NotePropertyName closeActor -NotePropertyValue $close13 -Force
if ($null -ne (Get-PendingVerification $issue13 $owner)) {
    throw "#13: a person-closed issue must never be re-checked"
}

# The bot-closed variant of the same history is the only closed shape that may
# be re-checked.
$close1301 = Get-LastCloseActor @($issue1301.events)
if ($close1301 -ne "github-actions[bot]") {
    throw "#1301: expected the last close by github-actions[bot], got '$close1301'"
}
$issue1301 | Add-Member -NotePropertyName closeActor -NotePropertyValue $close1301 -Force
$pending1301 = Get-PendingVerification $issue1301 $owner
if ($null -eq $pending1301 -or -not $pending1301.recheck) {
    throw "#1301: bot-closed re-check was not selected"
}
if ($pending1301.correction.id -ne "IC_kwDOVthgN88AAAABmiNpRQ") {
    throw "#1301: selected the wrong correction: $($pending1301.correction.id)"
}

# A verdict followed by coordinator chatter and no new correction must not be
# selected again. This was the fragile part of the old last-comment rule.
if ($null -ne (Get-PendingVerification $issue13Chatter $owner)) {
    throw "#13 chatter: verdict followed by a coordinator comment must not be re-selected"
}

# #9 was closed by the bot, but its verdict is newer than the correction.
$issue9 | Add-Member -NotePropertyName closeActor -NotePropertyValue (Get-LastCloseActor @($issue9.events)) -Force
if ($null -ne (Get-PendingVerification $issue9 $owner)) {
    throw "#9: closed issue with a newer verdict must not be selected"
}

# #3 was closed by a person and never had a verdict.
$issue3 | Add-Member -NotePropertyName closeActor -NotePropertyValue (Get-LastCloseActor @($issue3.events)) -Force
if ($null -ne (Get-PendingVerification $issue3 $owner)) {
    throw "#3: person-closed issue with no previous verdict must not be selected"
}

# #64 before the false-negative verdict: open and waiting for verification.
$pending64 = Get-PendingVerification $issue64 $owner
if ($null -eq $pending64) { throw "#64: open corrected issue was not selected" }
if ($pending64.recheck) { throw "#64: open issue should not be a re-check" }

function New-Verdict($Issue, [string]$Verdict, [object[]]$Expectations, [string]$Actual) {
    return [pscustomobject]@{
        issue = [int]$Issue.number
        issueTitle = $Issue.title
        issueBody = $Issue.body
        verdict = $Verdict
        title = "VEREDITO: $Verdict"
        kind = "observation"
        steps = @("Executei os passos da issue e conferi o resultado na interface.")
        expectations = $Expectations
        expected = "Os comportamentos esperados da issue funcionam."
        actual = $Actual
        screen = "bancada"
        windowSize = "1920x1080"
        correction = [pscustomobject]@{
            id = $Issue.comments[-1].id
            body = $Issue.comments[-1].body
            createdAt = $Issue.comments[-1].createdAt
            url = $Issue.comments[-1].url
        }
    }
}

$expected13 = @(
    [pscustomobject]@{ item = 'Ctrl+` alterna o painel do terminal'; ok = $true; observed = "O painel alternou" }
)
$failed13 = @(
    [pscustomobject]@{ item = 'Ctrl+` alterna o painel do terminal'; ok = $false; observed = "O painel não alternou" }
)
$expected1301 = @(
    [pscustomobject]@{ item = "O painel do terminal alterna"; ok = $false; observed = "O painel continuou aberto" }
)
$expected64 = @(
    [pscustomobject]@{ item = "Clicar no título do cartão abre a bancada"; ok = $true; observed = "A bancada Revisão API abriu" },
    [pscustomobject]@{ item = "Clicar no cartão não abre a janela de Ajustes"; ok = $true; observed = "Não abriu nenhuma janela de Ajustes" }
)

$fakeDir = Join-Path ([System.IO.Path]::GetTempPath()) "forja-fake-gh-$PID"
$oldPath = $env:PATH
$oldFixtures = $env:FORJA_GH_FIXTURES
$oldWrites = $env:FORJA_FAKE_GH_WRITES
try {
    $null = New-Item -ItemType Directory -Force $fakeDir
    Copy-Item (Join-Path $PSScriptRoot "fixtures/selection/fake-gh.ps1") $fakeDir
    $wrapper = Join-Path $fakeDir "gh.cmd"
    Set-Content -Path $wrapper -Encoding ascii -Value @"
@echo off
pwsh -NoProfile -File "%~dp0fake-gh.ps1" %*
"@
    $env:PATH = "$fakeDir;$oldPath"
    $env:FORJA_GH_FIXTURES = $issuesDir

    $selectedPath = Join-Path $fakeDir "verification-issues.json"
    $selectOutput = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "select-verifications.ps1") `
        -Repository "fixture/repo" -MaxIssues 8 -Out $selectedPath *>&1 | Out-String
    Write-Host $selectOutput
    $selected = @(Get-Content $selectedPath -Raw | ConvertFrom-Json)
    $numbers = @($selected | ForEach-Object { [int]$_.issue })
    if ($numbers.Count -ne 2) { throw "selection: expected 2 issues, got $($numbers -join ', ')" }
    if ($numbers[0] -ne 1301) { throw "selection: #1301 should be first (oldest), got $($numbers -join ', ')" }
    if ($numbers -notcontains 64) { throw "selection: #64 is missing: $($numbers -join ', ')" }
    if ($numbers -contains 13) { throw "selection: person-closed #13 must not be selected: $($numbers -join ', ')" }
    if ($numbers -contains 9) { throw "selection: #9 must not be selected: $($numbers -join ', ')" }
    if ($selectOutput -notmatch "skip #13: closed by guilhermelramos, not github-actions\[bot\]; not re-checking") {
        throw "selection: the person-closed skip was not logged"
    }
    Write-Host "selection using the real #13 history: $($numbers -join ', ') (person-closed #13 skipped)"

    # Person-closed #13, even with still_broken: it must be skipped with a log
    # and must not write anything. No -DryRun here, and the fake gh records any
    # comment, close, reopen, label, or edit the script attempts.
    $personBroken = New-Verdict $issue13 "still_broken" $failed13 "Ctrl+` não alternou o painel."
    $personBrokenPath = Join-Path $fakeDir "person-broken.jsonl"
    $personBroken | ConvertTo-Json -Compress -Depth 20 | Set-Content $personBrokenPath -Encoding utf8
    $writeLog = Join-Path $fakeDir "writes.txt"
    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $env:FORJA_FAKE_GH_WRITES = $writeLog
    try {
        $personBrokenRun = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
            -VerdictsPath $personBrokenPath -Repository "fixture/repo" *>&1 | Out-String
    } finally {
        Remove-Item Env:FORJA_FAKE_GH_WRITES -ErrorAction SilentlyContinue
    }
    if ($personBrokenRun -notmatch "skip #13: issue was closed by guilhermelramos, not github-actions\[bot\]; it will not be reopened or changed") {
        throw "apply: person-closed #13 was not skipped with a log"
    }
    if (Test-Path $writeLog) {
        $writes = (Get-Content $writeLog -Raw).Trim()
        if ($writes) { throw "apply: person-closed #13 wrote to GitHub: $writes" }
    }

    # Person-closed #13 with a fixed verdict must also be skipped, even in DryRun.
    $personFixed = New-Verdict $issue13 "fixed" $expected13 "O painel do terminal alternou com Ctrl+`."
    $personFixedPath = Join-Path $fakeDir "person-fixed.jsonl"
    $personFixed | ConvertTo-Json -Compress -Depth 20 | Set-Content $personFixedPath -Encoding utf8
    $personFixedRun = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
        -VerdictsPath $personFixedPath -Repository "fixture/repo" -DryRun *>&1 | Out-String
    if ($personFixedRun -match "DRY RUN #13") {
        throw "apply: person-closed #13 reached the dry-run plan"
    }
    if ($personFixedRun -notmatch "skip #13: issue was closed by guilhermelramos") {
        throw "apply: person-closed #13 fixed verdict was not logged as skipped"
    }

    # Bot-closed #1301 with still_broken: it may be reopened.
    $botBroken = New-Verdict $issue1301 "still_broken" $expected1301 "O painel continuou aberto."
    $botBrokenPath = Join-Path $fakeDir "bot-broken.jsonl"
    $botBroken | ConvertTo-Json -Compress -Depth 20 | Set-Content $botBrokenPath -Encoding utf8
    $botBrokenRun = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
        -VerdictsPath $botBrokenPath -Repository "fixture/repo" -DryRun *>&1 | Out-String
    Write-Host (($botBrokenRun -split "`n") | Where-Object { $_ -like "DRY RUN*" })
    if ($botBrokenRun -notmatch "DRY RUN #1301 \(still_broken\): would reopen the issue, comment, and add label 'reaberto'") {
        throw "apply: bot-closed #1301 should be reopened after still_broken"
    }

    # The real #64 evidence stays fixed: the word "não abriu" is expected.
    $case64 = New-Verdict $issue64 "fixed" $expected64 "A bancada Revisão API abriu. Não abriu nenhuma janela de Ajustes."
    $case64Path = Join-Path $fakeDir "case64.jsonl"
    $case64 | ConvertTo-Json -Compress -Depth 20 | Set-Content $case64Path -Encoding utf8
    $case64Run = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
        -VerdictsPath $case64Path -Repository "fixture/repo" -DryRun *>&1 | Out-String
    if ($case64Run -notmatch "DRY RUN #64 \(fixed\): would close the issue with this comment") {
        throw "apply: #64 should stay fixed"
    }
    if ($case64Run -match "#64: 'fixed' has false expectation") {
        throw "apply: #64 was downgraded again by failure words"
    }
    if ($case64Run -notmatch "#64: word heuristic matched a failure phrase, but every structured expectation is ok; keeping 'fixed'") {
        throw "apply: the word heuristic should be reported only as a log hint"
    }
} finally {
    $env:PATH = $oldPath
    if ($null -eq $oldFixtures) {
        Remove-Item Env:FORJA_GH_FIXTURES -ErrorAction SilentlyContinue
    } else {
        $env:FORJA_GH_FIXTURES = $oldFixtures
    }
    if ($null -eq $oldWrites) {
        Remove-Item Env:FORJA_FAKE_GH_WRITES -ErrorAction SilentlyContinue
    } else {
        $env:FORJA_FAKE_GH_WRITES = $oldWrites
    }
    Remove-Item $fakeDir -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "selection fixtures: person-closed #13 is skipped without writes; bot-closed #1301 is re-checked and can be reopened; #64 stays fixed"
