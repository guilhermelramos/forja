<##
.SYNOPSIS
  Aplica vereditos verificados às issues, ou mostra o plano com -DryRun.
#>
param(
    [Parameter(Mandatory)] [string]$VerdictsPath,
    [string]$Repository = $env:GH_REPO,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "verdict-rules.ps1")
if (-not $Repository) { throw "Repository is required (OWNER/REPO)" }
if (-not (Test-Path $VerdictsPath)) {
    Write-Host "no verdict file; nothing to apply"
    if ($env:GITHUB_OUTPUT) {
        "reopened_count=0" | Add-Content $env:GITHUB_OUTPUT
        "has_reopened=false" | Add-Content $env:GITHUB_OUTPUT
    }
    exit 0
}

$owner = (& gh api "repos/$Repository" --jq ".owner.login").Trim()
if ($LASTEXITCODE -ne 0 -or -not $owner) { throw "could not read repository owner" }
$verdicts = @(Get-Content $VerdictsPath | Where-Object { $_.Trim() } |
    ForEach-Object { $_ | ConvertFrom-Json })

function Get-LiveIssue([int]$Number) {
    $issue = & gh issue view $Number -R $Repository `
        --json state,closedAt,labels,comments,url,title,body | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "could not read issue #$Number" }
    if ($issue.state -eq "CLOSED") {
        # A closed issue is only a re-check target when the verifier closed it.
        $issue | Add-Member -NotePropertyName closeActor `
            -NotePropertyValue (Get-IssueCloseActor $Repository $Number) -Force
    }
    return $issue
}

function Test-LatestCorrection($Issue, $Result) {
    $pending = Get-PendingVerification $Issue $owner
    if ($null -eq $pending) { return $false }
    return (
        $pending.correction.id -eq $Result.correction.id -and
        $pending.correction.body -eq $Result.correction.body
    )
}

function Test-ClosedByPerson($Issue, [int]$Number) {
    if ($Issue.state -ne "CLOSED") { return $false }
    if ([string]$Issue.closeActor -eq $script:BotCloseActor) { return $false }
    $who = if ($Issue.closeActor) { $Issue.closeActor } else { "unknown" }
    Write-Host "skip #${Number}: issue was closed by $who, not github-actions[bot]; it will not be reopened or changed"
    return $true
}

$reopened = 0
try {
foreach ($result in $verdicts) {
    $number = [int]$result.issue
    $verdict = [string]$result.verdict
    if ($verdict -notin @("fixed", "still_broken", "cannot_reproduce")) {
        Write-Warning "skip #${number}: invalid verdict '$verdict'"
        continue
    }
    if ($result.title -ne "VEREDITO: $verdict" -or $result.kind -ne "observation" -or
        -not $result.steps -or -not $result.actual) {
        Write-Warning "skip #${number}: no explicit, evidence-backed verdict"
        continue
    }

    # The structured expectations list decides a `fixed` verdict. The word
    # heuristic is only a log hint for legacy verdicts without a list.
    $reportedVerdict = $verdict
    $expectations = @(Get-VerificationExpectations $result.expectations)
    $rawExpectations = $result.PSObject.Properties['expectations']
    if ($null -ne $rawExpectations -and $null -ne $rawExpectations.Value -and
        @($rawExpectations.Value).Count -gt 0 -and $expectations.Count -eq 0) {
        Write-Warning "#${number}: malformed expectations list was ignored"
    }

    $verdict = Get-EffectiveVerdict $verdict $result.expectations
    $downgraded = $verdict -ne $reportedVerdict
    if ($downgraded) {
        $failedItems = @($expectations | Where-Object { -not $_.ok } |
            ForEach-Object { $_.item }) -join "; "
        Write-Warning "#${number}: '$reportedVerdict' has false expectation(s) '$failedItems'; treating as '$verdict'"
    } elseif ($reportedVerdict -eq "fixed" -and $expectations.Count -gt 0) {
        if (Test-VerdictEvidenceFailure $result.steps $result.actual) {
            Write-Host "#${number}: word heuristic matched a failure phrase, but every structured expectation is ok; keeping 'fixed'"
        }
    } elseif ($reportedVerdict -eq "fixed") {
        $hint = if (Test-VerdictEvidenceFailure $result.steps $result.actual) {
            " (the word heuristic would have matched)"
        } else {
            ""
        }
        Write-Warning "#${number}: 'fixed' has no structured expectations; legacy verdict accepted$hint"
    }

    $issue = Get-LiveIssue $number
    if (Test-ClosedByPerson $issue $number) { continue }
    $pending = Get-PendingVerification $issue $owner
    if ($null -eq $pending) {
        if ($issue.state -ne "OPEN") {
            Write-Host "skip #${number}: issue is closed and has no correction after the close"
        } else {
            Write-Host "skip #${number}: correction changed or a newer verdict exists"
        }
        continue
    }
    $isRecheck = [bool]$pending.recheck
    $labels = @($issue.labels | ForEach-Object { $_.name })
    if (-not ($labels | Where-Object { $_ -in @("playtest", "explore", "ideia") })) {
        Write-Host "skip #${number}: verification label was removed"
        continue
    }
    if ($issue.title -ne $result.issueTitle -or $issue.body -ne $result.issueBody) {
        Write-Host "skip #${number}: issue text changed during verification"
        continue
    }
    if (-not (Test-LatestCorrection $issue $result)) {
        Write-Host "skip #${number}: correction changed or a newer verdict exists"
        continue
    }

    $prefix = switch ($verdict) {
        "fixed" { "Verificado: resolvido" }
        "still_broken" { "Verificado: ainda reproduz" }
        "cannot_reproduce" { "Verificado: não foi possível reproduzir" }
    }
    $stepLines = @($result.steps | ForEach-Object { "- $_" }) -join "`n"
    $expectationLines = if ($expectations.Count -gt 0) {
        $lines = @($expectations | ForEach-Object {
            $mark = if ($_.ok) { "x" } else { " " }
            "- [$mark] $($_.item) — $($_.observed)"
        }) -join "`n"
        "`nExpectativas:`n$lines"
    } else {
        ""
    }
    $runUrl = $env:GITHUB_SERVER_URL
    if ($env:GITHUB_REPOSITORY -and $env:GITHUB_RUN_ID) {
        $runUrl = "$runUrl/$env:GITHUB_REPOSITORY/actions/runs/$env:GITHUB_RUN_ID"
    }
    if (-not $runUrl) { $runUrl = "rodada local" }
    $downgradeNote = if ($downgraded) {
        $failedItems = @($expectations | Where-Object { -not $_.ok } |
            ForEach-Object { $_.item }) -join "; "
        "`nO veredito automático dizia '$reportedVerdict', mas a lista estruturada registra falha em: $failedItems."
    } else {
        ""
    }
    $windowLine = if ($result.windowSize) { "Janela: $($result.windowSize)`n" } else { "" }
    $comment = @"
$prefix

Passos e evidência:
$stepLines
$expectationLines
$windowLine
Esperado: $($result.expected)
Observado: $($result.actual)$downgradeNote
Rodada: $runUrl
"@

    if ($DryRun) {
        $action = switch ($verdict) {
            "fixed" {
                if ($isRecheck) {
                    "would comment on the closed issue confirming the fix"
                } else {
                    "would close the issue with this comment"
                }
            }
            "still_broken" {
                if ($isRecheck) {
                    $reopened++
                    "would reopen the issue, comment, and add label 'reaberto'"
                } else {
                    "would comment and add label 'reaberto'"
                }
            }
            "cannot_reproduce" {
                if ($isRecheck) {
                    $reopened++
                    "would reopen the issue, comment, and add label 'verificar-manual'"
                } else {
                    "would comment and add label 'verificar-manual'"
                }
            }
        }
        Write-Host "DRY RUN #$number ($verdict): $action"
        Write-Host $comment
        continue
    }

    # Re-read immediately before a write so a manual close during verification is respected.
    $issue = Get-LiveIssue $number
    if (Test-ClosedByPerson $issue $number) { continue }
    $pending = Get-PendingVerification $issue $owner
    if ($null -eq $pending -or $issue.title -ne $result.issueTitle -or
        $issue.body -ne $result.issueBody -or -not (Test-LatestCorrection $issue $result)) {
        Write-Host "skip #${number}: issue state or correction changed before update"
        continue
    }
    $isRecheck = [bool]$pending.recheck

    if ($isRecheck) {
        if ($verdict -eq "fixed") {
            gh issue comment $number -R $Repository --body $comment
            if ($LASTEXITCODE -ne 0) { throw "could not comment on closed issue #$number" }
            Write-Host "commented on closed #$number after fixed re-verification"
            continue
        }
        gh issue reopen $number -R $Repository
        if ($LASTEXITCODE -ne 0) { throw "could not reopen issue #$number" }
        $reopened++
        Write-Host "reopened #$number after '$verdict'"
    }

    if ($verdict -eq "fixed") {
        gh issue close $number -R $Repository --comment $comment
        if ($LASTEXITCODE -ne 0) { throw "could not close issue #$number with its verdict" }
        Write-Host "closed #$number after fixed verdict"
        continue
    }

    gh issue comment $number -R $Repository --body $comment
    if ($LASTEXITCODE -ne 0) { throw "could not comment on issue #$number" }
    $label = if ($verdict -eq "still_broken") { "reaberto" } else { "verificar-manual" }
    $description = if ($verdict -eq "still_broken") {
        "A correção ainda reproduz o problema"
    } else {
        "A verificação automática precisa de confirmação manual"
    }
    $color = if ($verdict -eq "still_broken") { "B60205" } else { "FBCA04" }
    gh label create $label -R $Repository --color $color --description $description --force
    if ($LASTEXITCODE -ne 0) { throw "could not create or update label '$label'" }
    gh issue edit $number -R $Repository --add-label $label
    if ($LASTEXITCODE -ne 0) { throw "could not add label '$label' to issue #$number" }
    Write-Host "commented and labeled #$number with '$label'"
}
} finally {
    if ($env:GITHUB_OUTPUT) {
        $hasReopened = if ($reopened -gt 0) { "true" } else { "false" }
        "reopened_count=$reopened" | Add-Content $env:GITHUB_OUTPUT
        "has_reopened=$hasReopened" | Add-Content $env:GITHUB_OUTPUT
    }
}
