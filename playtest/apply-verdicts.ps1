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
    exit 0
}

$owner = (& gh api "repos/$Repository" --jq ".owner.login").Trim()
if ($LASTEXITCODE -ne 0 -or -not $owner) { throw "could not read repository owner" }
$verdicts = @(Get-Content $VerdictsPath | Where-Object { $_.Trim() } |
    ForEach-Object { $_ | ConvertFrom-Json })

function Get-LiveIssue([int]$Number) {
    $issue = & gh issue view $Number -R $Repository `
        --json state,labels,comments,url,title,body | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "could not read issue #$Number" }
    return $issue
}

function Test-LatestCorrection($Issue, $Result) {
    $comments = @($Issue.comments | Sort-Object { [DateTimeOffset]$_.createdAt })
    $latestCorrection = $comments |
        Where-Object {
            $_.author.login -eq $owner -and
            ($_.body.StartsWith("Corrigido na main") -or
             $_.body.StartsWith("Tratado na main") -or
             $_.body.StartsWith("Concluído na main"))
        } |
        Select-Object -Last 1
    if (-not $latestCorrection -or $latestCorrection.id -ne $Result.correction.id -or
        $latestCorrection.body -ne $Result.correction.body) {
        return $false
    }
    $lastComment = $comments | Select-Object -Last 1
    if ($lastComment -and $lastComment.body.StartsWith("Verificado:") -and
        [DateTimeOffset]$lastComment.createdAt -ge [DateTimeOffset]$latestCorrection.createdAt) {
        return $false
    }
    return $true
}

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

    # A model can write `fixed` while its own evidence says the behavior did
    # not work. Evidence wins: downgrade before touching the issue.
    $reportedVerdict = $verdict
    $verdict = Get-EffectiveVerdict $verdict $result.steps $result.actual
    $downgraded = $verdict -ne $reportedVerdict
    if ($downgraded) {
        Write-Warning "#${number}: '$reportedVerdict' has failure evidence; treating as '$verdict'"
    }

    $issue = Get-LiveIssue $number
    if ($issue.state -ne "OPEN") {
        Write-Host "skip #${number}: issue is closed; it will not be reopened or changed"
        continue
    }
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
    $runUrl = $env:GITHUB_SERVER_URL
    if ($env:GITHUB_REPOSITORY -and $env:GITHUB_RUN_ID) {
        $runUrl = "$runUrl/$env:GITHUB_REPOSITORY/actions/runs/$env:GITHUB_RUN_ID"
    }
    if (-not $runUrl) { $runUrl = "rodada local" }
    $downgradeNote = if ($downgraded) {
        "`nO veredito automático dizia '$reportedVerdict', mas a evidência registra uma falha; por isso foi tratado como '$verdict'."
    } else {
        ""
    }
    $windowLine = if ($result.windowSize) { "Janela: $($result.windowSize)`n" } else { "" }
    $comment = @"
$prefix

Passos e evidência:
$stepLines
$windowLine
Esperado: $($result.expected)
Observado: $($result.actual)$downgradeNote
Rodada: $runUrl
"@

    if ($DryRun) {
        $action = switch ($verdict) {
            "fixed" { "would close the issue with this comment" }
            "still_broken" { "would comment and add label 'reaberto'" }
            "cannot_reproduce" { "would comment and add label 'verificar-manual'" }
        }
        Write-Host "DRY RUN #$number ($verdict): $action"
        Write-Host $comment
        continue
    }

    # Re-read immediately before a write so a manual close during verification is respected.
    $issue = Get-LiveIssue $number
    if ($issue.state -ne "OPEN" -or $issue.title -ne $result.issueTitle -or
        $issue.body -ne $result.issueBody -or -not (Test-LatestCorrection $issue $result)) {
        Write-Host "skip #${number}: issue state or correction changed before update"
        continue
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
