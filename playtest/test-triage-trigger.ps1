# Fixture tests for conditional triage workflow dispatch outputs in
# playtest/report.ps1 and playtest/apply-verdicts.ps1, plus workflow contracts.
# Run: pwsh -NoProfile -File ./playtest/test-triage-trigger.ps1

$ErrorActionPreference = "Stop"

function Read-GitHubOutputs([string]$Path) {
    $map = @{}
    if (-not (Test-Path $Path)) { return $map }
    foreach ($line in Get-Content $Path) {
        if ($line -match '^([^=]+)=(.*)$') {
            $map[$Matches[1]] = $Matches[2]
        }
    }
    return $map
}

$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) "forja-triage-trigger-test-$PID"
$oldPath = $env:PATH
$oldFixtures = $env:FORJA_GH_FIXTURES
$oldWrites = $env:FORJA_FAKE_GH_WRITES
$oldOutput = $env:GITHUB_OUTPUT
$oldLocation = Get-Location

try {
    $null = New-Item -ItemType Directory -Force $tempDir
    Set-Location $tempDir

    # 1. report.ps1 -DryRun with 1 finding -> has_created=true, created_count=1
    $findingsIn = Join-Path $tempDir "in-one"
    $missionDir = Join-Path $findingsIn "playtest-caos"
    $null = New-Item -ItemType Directory -Force $missionDir
    '{"title":"Botão não responde ao Enter","severity":"high","kind":"bug","screen":"ajustes","steps":["Abrir Ajustes","Pressionar Enter"],"expected":"Acionar botão","actual":"Nada acontece"}' |
        Set-Content (Join-Path $missionDir "findings.jsonl") -Encoding utf8

    $outOne = Join-Path $tempDir "out-report-one.txt"
    $env:GITHUB_OUTPUT = $outOne
    $dryOne = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "report.ps1") `
        -In $findingsIn -MaxIssues 10 -Report (Join-Path $tempDir "report-one.md") -DryRun *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "report.ps1 DryRun (1 finding) failed:`n$dryOne" }
    $mapOne = Read-GitHubOutputs $outOne
    if ($mapOne["created_count"] -ne "1" -or $mapOne["has_created"] -ne "true") {
        throw "expected created_count=1 and has_created=true, got: $($mapOne | ConvertTo-Json -Compress)"
    }

    # 2. report.ps1 -DryRun with 0 findings -> has_created=false, created_count=0
    $findingsEmpty = Join-Path $tempDir "in-empty"
    $null = New-Item -ItemType Directory -Force $findingsEmpty
    $outEmpty = Join-Path $tempDir "out-report-empty.txt"
    $env:GITHUB_OUTPUT = $outEmpty
    $dryEmpty = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "report.ps1") `
        -In $findingsEmpty -MaxIssues 10 -Report (Join-Path $tempDir "report-empty.md") -DryRun *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "report.ps1 DryRun (0 findings) failed:`n$dryEmpty" }
    $mapEmpty = Read-GitHubOutputs $outEmpty
    if ($mapEmpty["created_count"] -ne "0" -or $mapEmpty["has_created"] -ne "false") {
        throw "expected created_count=0 and has_created=false, got: $($mapEmpty | ConvertTo-Json -Compress)"
    }

    # 3. report.ps1 -DryRun with MaxIssues 0 -> has_created=false, created_count=0
    $outZeroMax = Join-Path $tempDir "out-report-zeromax.txt"
    $env:GITHUB_OUTPUT = $outZeroMax
    $dryZeroMax = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "report.ps1") `
        -In $findingsIn -MaxIssues 0 -Report (Join-Path $tempDir "report-zeromax.md") -DryRun *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "report.ps1 DryRun (MaxIssues 0) failed:`n$dryZeroMax" }
    $mapZeroMax = Read-GitHubOutputs $outZeroMax
    if ($mapZeroMax["created_count"] -ne "0" -or $mapZeroMax["has_created"] -ne "false") {
        throw "expected created_count=0 and has_created=false for MaxIssues=0, got: $($mapZeroMax | ConvertTo-Json -Compress)"
    }

    # 4. report.ps1 live mode with fake gh: duplicate skipped, new created -> has_created=true; only duplicate -> has_created=false
    $fakeReportDir = Join-Path $tempDir "fake-report-gh"
    $null = New-Item -ItemType Directory -Force $fakeReportDir
    Set-Content -Path (Join-Path $fakeReportDir "fake-gh-report.ps1") -Encoding utf8 -Value @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
$ErrorActionPreference = "Stop"
if ($Arguments[0] -eq "label" -and $Arguments[1] -eq "create") {
    exit 0
}
if ($Arguments[0] -eq "issue" -and $Arguments[1] -eq "list") {
    '[{"title":"[playtest] Bug já registrado antes"}]'
    exit 0
}
if ($Arguments[0] -eq "issue" -and $Arguments[1] -eq "create") {
    if ($env:FORJA_FAKE_GH_WRITES) {
        Add-Content -Path $env:FORJA_FAKE_GH_WRITES -Value ($Arguments -join " ") -Encoding utf8
    }
    "https://github.com/fixture/repo/issues/999"
    exit 0
}
throw "unexpected fake gh call: $($Arguments -join ' ')"
'@
    Set-Content -Path (Join-Path $fakeReportDir "gh.cmd") -Encoding ascii -Value @"
@echo off
pwsh -NoProfile -File "%~dp0fake-gh-report.ps1" %*
"@
    $env:PATH = "$fakeReportDir;$oldPath"

    $findingsDupAndNew = Join-Path $tempDir "in-dup-new"
    $missionDupNew = Join-Path $findingsDupAndNew "playtest-caos"
    $null = New-Item -ItemType Directory -Force $missionDupNew
    @(
        '{"title":"Bug já registrado antes","severity":"medium","kind":"bug","screen":"ajustes","steps":["Abrir"],"expected":"Ok","actual":"Falha"}'
        '{"title":"Novo achado exclusivo","severity":"high","kind":"bug","screen":"editor","steps":["Editar"],"expected":"Salva","actual":"Erro"}'
    ) | Set-Content (Join-Path $missionDupNew "findings.jsonl") -Encoding utf8

    $reportWrites = Join-Path $tempDir "report-writes.txt"
    $env:FORJA_FAKE_GH_WRITES = $reportWrites
    $outLiveNew = Join-Path $tempDir "out-report-live-new.txt"
    $env:GITHUB_OUTPUT = $outLiveNew
    $liveNewOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "report.ps1") `
        -In $findingsDupAndNew -MaxIssues 10 -Report (Join-Path $tempDir "report-live-new.md") *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "report.ps1 live (dup+new) failed:`n$liveNewOut" }
    $mapLiveNew = Read-GitHubOutputs $outLiveNew
    if ($mapLiveNew["created_count"] -ne "1" -or $mapLiveNew["has_created"] -ne "true") {
        throw "expected created_count=1 and has_created=true, got: $($mapLiveNew | ConvertTo-Json -Compress)"
    }
    $createdCalls = @(Get-Content $reportWrites)
    if ($createdCalls.Count -ne 1 -or $createdCalls[0] -notmatch "Novo achado exclusivo") {
        throw "expected exactly 1 gh issue create for 'Novo achado exclusivo', got: $($createdCalls -join '; ')"
    }

    $findingsOnlyDup = Join-Path $tempDir "in-only-dup"
    $missionOnlyDup = Join-Path $findingsOnlyDup "playtest-caos"
    $null = New-Item -ItemType Directory -Force $missionOnlyDup
    '{"title":"Bug já registrado antes","severity":"medium","kind":"bug","screen":"ajustes","steps":["Abrir"],"expected":"Ok","actual":"Falha"}' |
        Set-Content (Join-Path $missionOnlyDup "findings.jsonl") -Encoding utf8

    Remove-Item $reportWrites -ErrorAction SilentlyContinue
    $outLiveDup = Join-Path $tempDir "out-report-live-dup.txt"
    $env:GITHUB_OUTPUT = $outLiveDup
    $liveDupOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "report.ps1") `
        -In $findingsOnlyDup -MaxIssues 10 -Report (Join-Path $tempDir "report-live-dup.md") *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "report.ps1 live (only dup) failed:`n$liveDupOut" }
    $mapLiveDup = Read-GitHubOutputs $outLiveDup
    if ($mapLiveDup["created_count"] -ne "0" -or $mapLiveDup["has_created"] -ne "false") {
        throw "expected created_count=0 and has_created=false for duplicate-only run, got: $($mapLiveDup | ConvertTo-Json -Compress)"
    }
    if (Test-Path $reportWrites) {
        throw "duplicate-only run should not invoke gh issue create"
    }

    # 5. apply-verdicts.ps1 with selection fake gh
    $fakeApplyDir = Join-Path $tempDir "fake-apply-gh"
    $null = New-Item -ItemType Directory -Force $fakeApplyDir
    Copy-Item (Join-Path $PSScriptRoot "fixtures/selection/fake-gh.ps1") $fakeApplyDir
    Set-Content -Path (Join-Path $fakeApplyDir "gh.cmd") -Encoding ascii -Value @"
@echo off
pwsh -NoProfile -File "%~dp0fake-gh.ps1" %*
"@
    $env:PATH = "$fakeApplyDir;$oldPath"
    $issuesDir = Join-Path $PSScriptRoot "fixtures/selection/issues"
    $env:FORJA_GH_FIXTURES = $issuesDir
    Remove-Item Env:FORJA_FAKE_GH_WRITES -ErrorAction SilentlyContinue

    $issue1301 = Get-Content (Join-Path $issuesDir "issue-1301.json") -Raw | ConvertFrom-Json
    $issue64 = Get-Content (Join-Path $issuesDir "issue-64.json") -Raw | ConvertFrom-Json
    $issue13 = Get-Content (Join-Path $issuesDir "issue-13.json") -Raw | ConvertFrom-Json

    function New-TestVerdict($Issue, [string]$Verdict, [object[]]$Expectations, [string]$Actual) {
        return [pscustomobject]@{
            issue = [int]$Issue.number
            issueTitle = $Issue.title
            issueBody = $Issue.body
            verdict = $Verdict
            title = "VEREDITO: $Verdict"
            kind = "observation"
            steps = @("Passos executados.")
            expectations = $Expectations
            expected = "Comportamento esperado."
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

    # 5a. Missing verdicts file -> has_reopened=false, reopened_count=0
    $outMissing = Join-Path $tempDir "out-apply-missing.txt"
    $env:GITHUB_OUTPUT = $outMissing
    $missingOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
        -VerdictsPath (Join-Path $tempDir "does-not-exist.jsonl") -Repository "fixture/repo" *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "apply-verdicts.ps1 (missing file) failed:`n$missingOut" }
    $mapMissing = Read-GitHubOutputs $outMissing
    if ($mapMissing["reopened_count"] -ne "0" -or $mapMissing["has_reopened"] -ne "false") {
        throw "expected reopened_count=0 and has_reopened=false for missing verdicts file, got: $($mapMissing | ConvertTo-Json -Compress)"
    }

    # 5b. Bot-closed #1301 still_broken (-DryRun and live) -> has_reopened=true, reopened_count=1
    $v1301Broken = New-TestVerdict $issue1301 "still_broken" @(
        [pscustomobject]@{ item = "Painel alterna"; ok = $false; observed = "Continuou aberto" }
    ) "Continuou aberto."
    $v1301BrokenPath = Join-Path $tempDir "v1301-broken.jsonl"
    $v1301Broken | ConvertTo-Json -Compress -Depth 20 | Set-Content $v1301BrokenPath -Encoding utf8

    $outDryReopen = Join-Path $tempDir "out-apply-dry-reopen.txt"
    $env:GITHUB_OUTPUT = $outDryReopen
    $dryReopenOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
        -VerdictsPath $v1301BrokenPath -Repository "fixture/repo" -DryRun *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "apply-verdicts.ps1 DryRun (#1301 reopen) failed:`n$dryReopenOut" }
    $mapDryReopen = Read-GitHubOutputs $outDryReopen
    if ($mapDryReopen["reopened_count"] -ne "1" -or $mapDryReopen["has_reopened"] -ne "true") {
        throw "expected reopened_count=1 and has_reopened=true in DryRun, got: $($mapDryReopen | ConvertTo-Json -Compress)"
    }

    $applyWrites = Join-Path $tempDir "apply-writes.txt"
    $env:FORJA_FAKE_GH_WRITES = $applyWrites
    $outLiveReopen = Join-Path $tempDir "out-apply-live-reopen.txt"
    $env:GITHUB_OUTPUT = $outLiveReopen
    $liveReopenOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
        -VerdictsPath $v1301BrokenPath -Repository "fixture/repo" *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "apply-verdicts.ps1 live (#1301 reopen) failed:`n$liveReopenOut" }
    $mapLiveReopen = Read-GitHubOutputs $outLiveReopen
    if ($mapLiveReopen["reopened_count"] -ne "1" -or $mapLiveReopen["has_reopened"] -ne "true") {
        throw "expected reopened_count=1 and has_reopened=true in live run, got: $($mapLiveReopen | ConvertTo-Json -Compress)"
    }
    if (@(Get-Content $applyWrites) -notcontains "reopen 1301") {
        throw "expected 'reopen 1301' in fake gh writes: $((Get-Content $applyWrites) -join ', ')"
    }

    # 5c. Bot-closed #1301 fixed (live) -> does not reopen -> has_reopened=false, reopened_count=0
    $v1301Fixed = New-TestVerdict $issue1301 "fixed" @(
        [pscustomobject]@{ item = "Painel alterna"; ok = $true; observed = "Alternou" }
    ) "Alternou."
    $v1301FixedPath = Join-Path $tempDir "v1301-fixed.jsonl"
    $v1301Fixed | ConvertTo-Json -Compress -Depth 20 | Set-Content $v1301FixedPath -Encoding utf8

    Remove-Item $applyWrites -ErrorAction SilentlyContinue
    $outLiveFixed = Join-Path $tempDir "out-apply-live-fixed.txt"
    $env:GITHUB_OUTPUT = $outLiveFixed
    $liveFixedOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
        -VerdictsPath $v1301FixedPath -Repository "fixture/repo" *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "apply-verdicts.ps1 live (#1301 fixed) failed:`n$liveFixedOut" }
    $mapLiveFixed = Read-GitHubOutputs $outLiveFixed
    if ($mapLiveFixed["reopened_count"] -ne "0" -or $mapLiveFixed["has_reopened"] -ne "false") {
        throw "expected reopened_count=0 and has_reopened=false when recheck is fixed, got: $($mapLiveFixed | ConvertTo-Json -Compress)"
    }
    if (@(Get-Content $applyWrites) -match "^reopen ") {
        throw "fixed recheck must not reopen issue"
    }

    # 5d. Open #64 fixed and person-closed #13 still_broken (-DryRun) -> has_reopened=false
    $v64 = New-TestVerdict $issue64 "fixed" @(
        [pscustomobject]@{ item = "Bancada abre"; ok = $true; observed = "Abriu" }
    ) "Abriu."
    $v13 = New-TestVerdict $issue13 "still_broken" @(
        [pscustomobject]@{ item = "Alterna"; ok = $false; observed = "Não alternou" }
    ) "Não alternou."
    $vNoReopenPath = Join-Path $tempDir "v-no-reopen.jsonl"
    @(
        ($v64 | ConvertTo-Json -Compress -Depth 20)
        ($v13 | ConvertTo-Json -Compress -Depth 20)
    ) | Set-Content $vNoReopenPath -Encoding utf8

    $outDryNoReopen = Join-Path $tempDir "out-apply-dry-noreopen.txt"
    $env:GITHUB_OUTPUT = $outDryNoReopen
    $dryNoReopenOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "apply-verdicts.ps1") `
        -VerdictsPath $vNoReopenPath -Repository "fixture/repo" -DryRun *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "apply-verdicts.ps1 DryRun (no reopen) failed:`n$dryNoReopenOut" }
    $mapDryNoReopen = Read-GitHubOutputs $outDryNoReopen
    if ($mapDryNoReopen["reopened_count"] -ne "0" -or $mapDryNoReopen["has_reopened"] -ne "false") {
        throw "expected reopened_count=0 and has_reopened=false for #64+#13, got: $($mapDryNoReopen | ConvertTo-Json -Compress)"
    }

    # 6. Workflow contract checks
    $repoRoot = Split-Path -Parent $PSScriptRoot
    foreach ($wf in @(
        @{ File = "playtest.yml"; Condition = "steps\.report\.outputs\.has_created == 'true'" },
        @{ File = "ideas.yml"; Condition = "steps\.report\.outputs\.has_created == 'true'" },
        @{ File = "explore.yml"; Condition = "steps\.report\.outputs\.has_created == 'true'" },
        @{ File = "verify.yml"; Condition = "steps\.apply\.outputs\.has_reopened == 'true'" }
    )) {
        $wfPath = Join-Path $repoRoot ".github/workflows/$($wf.File)"
        $raw = Get-Content $wfPath -Raw
        if ($raw -notmatch "(?m)^\s+actions:\s+write\s*$") {
            throw "$($wf.File): missing 'actions: write' permission"
        }
        if ($raw -notmatch $wf.Condition) {
            throw "$($wf.File): missing conditional triage check '$($wf.Condition)'"
        }
        if ($raw -notmatch "gh workflow run triage\.yml") {
            throw "$($wf.File): missing 'gh workflow run triage.yml'"
        }
    }
} finally {
    Set-Location $oldLocation
    $env:PATH = $oldPath
    if ($null -eq $oldFixtures) { Remove-Item Env:FORJA_GH_FIXTURES -ErrorAction SilentlyContinue } else { $env:FORJA_GH_FIXTURES = $oldFixtures }
    if ($null -eq $oldWrites) { Remove-Item Env:FORJA_FAKE_GH_WRITES -ErrorAction SilentlyContinue } else { $env:FORJA_FAKE_GH_WRITES = $oldWrites }
    if ($null -eq $oldOutput) { Remove-Item Env:GITHUB_OUTPUT -ErrorAction SilentlyContinue } else { $env:GITHUB_OUTPUT = $oldOutput }
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "triage trigger fixtures: report.ps1, apply-verdicts.ps1, and workflow contracts passed"
