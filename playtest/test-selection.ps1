# Fixture test for issue selection and the end-to-end -DryRun flow.
# It uses the real history of #13 (closed 13 seconds before the new correction)
# and the real #64 evidence that the old word heuristic downgraded by mistake.
# Run: ./playtest/test-selection.ps1

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "verdict-rules.ps1")

$owner = "guilhermelramos"
$issuesDir = Join-Path $PSScriptRoot "fixtures/selection/issues"
$issue13 = Get-Content (Join-Path $issuesDir "issue-13.json") -Raw | ConvertFrom-Json
$issue13Chatter = Get-Content (Join-Path $issuesDir "chatter-13.json") -Raw | ConvertFrom-Json
$issue64 = Get-Content (Join-Path $issuesDir "issue-64.json") -Raw | ConvertFrom-Json
$issue9 = Get-Content (Join-Path $issuesDir "issue-9.json") -Raw | ConvertFrom-Json
$issue3 = Get-Content (Join-Path $issuesDir "issue-3.json") -Raw | ConvertFrom-Json

# #13 was closed at 16:50:58 and the coordinator posted the new correction at
# 16:51:11. It must be selected as a re-check even though it is closed.
$pending13 = Get-PendingVerification $issue13 $owner
if ($null -eq $pending13) { throw "#13: closed re-check was not selected" }
if (-not $pending13.recheck) { throw "#13: closed issue should be marked as a re-check" }
if ($pending13.correction.id -ne "IC_kwDOVthgN88AAAABmiNpRQ") {
    throw "#13: selected the wrong correction: $($pending13.correction.id)"
}

# A verdict followed by coordinator chatter and no new correction must not be
# selected again. This was the fragile part of the old last-comment rule.
if ($null -ne (Get-PendingVerification $issue13Chatter $owner)) {
    throw "#13 chatter: verdict followed by a coordinator comment must not be re-selected"
}

# A closed issue whose verdict is newer than the correction is already done.
if ($null -ne (Get-PendingVerification $issue9 $owner)) {
    throw "#9: closed issue with a newer verdict must not be selected"
}

# A closed issue without any verdict was closed manually before its first
# correction; it must stay closed instead of looking like a re-check.
if ($null -ne (Get-PendingVerification $issue3 $owner)) {
    throw "#3: manually closed issue with no previous verdict must not be selected"
}

# #64 before the false-negative verdict: open and waiting for verification.
$pending64 = Get-PendingVerification $issue64 $owner
if ($null -eq $pending64) { throw "#64: open corrected issue was not selected" }
if ($pending64.recheck) { throw "#64: open issue should not be a re-check" }

$fakeDir = Join-Path ([System.IO.Path]::GetTempPath()) "forja-fake-gh-$PID"
$oldPath = $env:PATH
$oldFixtures = $env:FORJA_GH_FIXTURES
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
    & (Join-Path $PSScriptRoot "select-verifications.ps1") `
        -Repository "fixture/repo" -MaxIssues 8 -Out $selectedPath
    $selected = @(Get-Content $selectedPath -Raw | ConvertFrom-Json)
    $numbers = @($selected | ForEach-Object { [int]$_.issue })
    if ($numbers.Count -ne 2) { throw "selection: expected 2 issues, got $($numbers -join ', ')" }
    if ($numbers[0] -ne 13) { throw "selection: #13 should be first (oldest), got $($numbers -join ', ')" }
    if ($numbers -notcontains 64) { throw "selection: #64 is missing: $($numbers -join ', ')" }
    if ($numbers -contains 9) { throw "selection: closed #9 must not be selected: $($numbers -join ', ')" }
    if (-not ($selected | Where-Object { $_.issue -eq 13 }).recheck) {
        throw "selection: #13 should carry recheck=true"
    }
    Write-Host "selection using the real #13 history: $($numbers -join ', ')"

    $verdicts = @(
        [pscustomobject]@{
            issue = 13
            issueTitle = $issue13.title
            issueBody = $issue13.body
            verdict = "fixed"
            title = "VEREDITO: fixed"
            kind = "observation"
            steps = @(
                "Fechei a aba do terminal com Ctrl+Shift+W e o painel desapareceu.",
                'Pressionei Ctrl+` com o terminal em foco e o painel alternou.'
            )
            expectations = @(
                [pscustomobject]@{ item = 'Ctrl+` alterna o painel do terminal'; ok = $true; observed = "O painel inferior apareceu e sumiu" },
                [pscustomobject]@{ item = "Abrir terminal reabre o painel com uma nova aba"; ok = $true; observed = "O painel reabriu com uma aba pwsh" }
            )
            expected = 'Ctrl+` alterna o painel do terminal.'
            actual = 'O painel alternou com Ctrl+` e reabriu pelo comando Abrir terminal.'
            screen = "bancada com terminal"
            windowSize = "1920x1080"
            correction = [pscustomobject]@{
                id = $issue13.comments[-1].id
                body = $issue13.comments[-1].body
                createdAt = $issue13.comments[-1].createdAt
                url = $issue13.comments[-1].url
            }
        },
        [pscustomobject]@{
            issue = 64
            issueTitle = $issue64.title
            issueBody = $issue64.body
            verdict = "fixed"
            title = "VEREDITO: fixed"
            kind = "observation"
            steps = @(
                "Abri o Quadro e cliquei no título do cartão Revisão API.",
                "A bancada Revisão API abriu; nenhuma janela de Ajustes apareceu."
            )
            expectations = @(
                [pscustomobject]@{ item = "Clicar no título do cartão abre a bancada"; ok = $true; observed = "A bancada Revisão API abriu" },
                [pscustomobject]@{ item = "Clicar no cartão não abre a janela de Ajustes"; ok = $true; observed = "Não abriu nenhuma janela de Ajustes" }
            )
            expected = "O cartão abre a bancada em vez da janela global de Ajustes."
            actual = "A bancada Revisão API abriu. Não abriu nenhuma janela de Ajustes."
            screen = "Quadro"
            windowSize = "1920x1080"
            correction = [pscustomobject]@{
                id = $issue64.comments[-1].id
                body = $issue64.comments[-1].body
                createdAt = $issue64.comments[-1].createdAt
                url = $issue64.comments[-1].url
            }
        }
    )
    $verdictsPath = Join-Path $fakeDir "verdicts.jsonl"
    $verdicts | ForEach-Object { $_ | ConvertTo-Json -Compress -Depth 16 } |
        Set-Content $verdictsPath -Encoding utf8

    $dryRun = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
        -VerdictsPath $verdictsPath -Repository "fixture/repo" -DryRun *>&1 | Out-String
    Write-Host $dryRun
    if ($dryRun -notmatch "DRY RUN #13 \(fixed\): would comment on the closed issue confirming the fix") {
        throw "dry run: #13 should confirm the closed issue instead of closing it again"
    }
    if ($dryRun -notmatch "DRY RUN #64 \(fixed\): would close the issue with this comment") {
        throw "dry run: #64 should stay fixed"
    }
    if ($dryRun -match "#64: 'fixed' has false expectation") {
        throw "dry run: #64 was downgraded again by failure words"
    }
    if ($dryRun -notmatch "#64: word heuristic matched a failure phrase, but every structured expectation is ok; keeping 'fixed'") {
        throw "dry run: the word heuristic should be reported only as a log hint"
    }

    # A failed re-check of the closed #13 must reopen it instead of being skipped.
    $failed = $verdicts[0].PSObject.Copy()
    $failed.verdict = "still_broken"
    $failed.title = "VEREDITO: still_broken"
    $failed.expectations = @(
        [pscustomobject]@{ item = 'Ctrl+` alterna o painel do terminal'; ok = $false; observed = "O painel não alternou" }
    )
    $failed.actual = 'Ctrl+` não alternou o painel do terminal.'
    $failedPath = Join-Path $fakeDir "failed-verdict.jsonl"
    $failed | ConvertTo-Json -Compress -Depth 16 | Set-Content $failedPath -Encoding utf8
    $failedRun = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
        -VerdictsPath $failedPath -Repository "fixture/repo" -DryRun *>&1 | Out-String
    if ($failedRun -notmatch "DRY RUN #13 \(still_broken\): would reopen the issue, comment, and add label 'reaberto'") {
        throw "dry run: a failed re-check of closed #13 must reopen it"
    }
} finally {
    $env:PATH = $oldPath
    if ($null -eq $oldFixtures) {
        Remove-Item Env:FORJA_GH_FIXTURES -ErrorAction SilentlyContinue
    } else {
        $env:FORJA_GH_FIXTURES = $oldFixtures
    }
    Remove-Item $fakeDir -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "selection fixtures: real #13 closed re-check passes; #64 structured expectations keep fixed"
