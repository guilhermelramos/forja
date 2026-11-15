<#
.SYNOPSIS
  Automatic issue triager: assigns priority (1-5) and area labels to open issues.

.DESCRIPTION
  Selects open issues without a `prioridade-N` label (or all open issues when
  `-Retriage` is set), evaluates deterministic pre-AI shortcuts, asks a free
  OpenCode model to classify priority, area, rationale, and possible duplicate,
  validates the JSON response, applies labels and a single-line comment (or
  shows the plan with `-DryRun`). Never closes issues, never touches closed
  issues, and never adds or removes `em-andamento`.
#>
param(
    [string]$Repository = $env:GH_REPO,
    [int]$MaxIssues = 30,
    [int]$IssueNumber = 0,
    [switch]$Retriage,
    [switch]$DryRun,
    [string[]]$Models = @(),
    [int]$TimeoutSeconds = 90
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "triage-rules.ps1")

if ($MaxIssues -lt 1) { throw "MaxIssues must be at least 1" }
if ($TimeoutSeconds -lt 1) { throw "TimeoutSeconds must be at least 1" }

if (-not $Repository) {
    $Repository = (& gh repo view --json nameWithOwner --jq .nameWithOwner 2>$null | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $Repository) {
        throw "Repository is required (OWNER/REPO)"
    }
}

if (-not $Models.Count -and $env:TRIAGE_MODELS) {
    $Models = @($env:TRIAGE_MODELS -split ",")
}
if (-not $Models.Count) {
    $Models = @(
        "openrouter/nvidia/nemotron-3.5-lightning:free",
        "openrouter/nvidia/nemotron-3-ultra-550b-a55b:free",
        "openrouter/google/gemma-4-31b-it:free"
    )
}
$Models = @($Models | ForEach-Object { $_ -split "," } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
if (-not $Models.Count) { throw "At least one free model is required" }

$invalidModels = @($Models | Where-Object { $_ -notmatch "(-free|:free)$" })
if ($invalidModels.Count) {
    throw "Only free models are allowed; invalid model id(s): $($invalidModels -join ', ')"
}
$hasOpenRouterModel = @($Models | Where-Object { $_.StartsWith("openrouter/") }).Count -gt 0
if ($hasOpenRouterModel -and -not $env:OPENROUTER_API_KEY) {
    throw "OPENROUTER_API_KEY is required for OpenRouter models; set TRIAGE_MODELS to another free provider for a local run"
}

function Resolve-OpenCodeRunner {
    if ($env:FORJA_OPENCODE_SCRIPT) {
        if (-not (Test-Path $env:FORJA_OPENCODE_SCRIPT)) {
            throw "FORJA_OPENCODE_SCRIPT does not exist: $($env:FORJA_OPENCODE_SCRIPT)"
        }
        $pwshCmd = (Get-Command pwsh -CommandType Application | Select-Object -First 1).Source
        return [pscustomobject]@{
            Executable = $pwshCmd
            PrefixArgs = @("-NoProfile", "-File", $env:FORJA_OPENCODE_SCRIPT)
        }
    }

    if ($env:OS -eq "Windows_NT" -and (Get-Command npm -ErrorAction SilentlyContinue)) {
        $npmRoot = (& npm root -g 2>$null | Out-String).Trim()
        if ($npmRoot) {
            $exe = Join-Path $npmRoot "@opencode/cli/bin/opencode.exe"
            if (Test-Path $exe) {
                return [pscustomobject]@{ Executable = $exe; PrefixArgs = @() }
            }
        }
    }

    $cmd = Get-Command opencode -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $cmd) {
        throw "OpenCode CLI was not found on PATH"
    }
    return [pscustomobject]@{ Executable = $cmd.Source; PrefixArgs = @() }
}

function Invoke-OpenCodeTriage($Runner, [string]$Model, [string]$Prompt, [int]$TimeoutSec) {
    $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("forja-triage-" + [guid]::NewGuid().ToString("N"))
    $room = Join-Path $tempRoot "room"
    $configHome = Join-Path $tempRoot "config"
    $privateHome = Join-Path $tempRoot "home"
    $null = New-Item -ItemType Directory -Force $room, $configHome, $privateHome

    try {
        $config = [ordered]@{
            '$schema' = "https://opencode.ai/config.json"
            permissions = @([pscustomobject]@{ action = "*"; resource = "*"; effect = "deny" })
        }
        $config | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $room "opencode.json") -Encoding utf8

        $psi = [System.Diagnostics.ProcessStartInfo]::new($Runner.Executable)
        foreach ($arg in $Runner.PrefixArgs) { $psi.ArgumentList.Add($arg) }
        foreach ($arg in @("run", "--standalone", "--auto", "--archived", "--model", $Model, $Prompt)) {
            $psi.ArgumentList.Add($arg)
        }
        $psi.WorkingDirectory = $room
        $psi.Environment["USERPROFILE"] = $privateHome
        $psi.Environment["HOME"] = $privateHome
        $psi.Environment["APPDATA"] = [System.IO.Path]::Combine($privateHome, "AppData", "Roaming")
        $psi.Environment["LOCALAPPDATA"] = [System.IO.Path]::Combine($privateHome, "AppData", "Local")
        $psi.Environment["XDG_CONFIG_HOME"] = $configHome
        $psi.Environment["XDG_DATA_HOME"] = Join-Path $configHome "data"
        $psi.Environment["XDG_CACHE_HOME"] = Join-Path $configHome "cache"
        foreach ($name in @(
            "GITHUB_TOKEN", "GH_TOKEN", "ACTIONS_RUNTIME_TOKEN",
            "OPENCODE_CONFIG", "OPENCODE_CONFIG_DIR", "OPENCODE_CONFIG_CONTENT", "OPENCODE_CLI_CONFIG_CONTENT"
        )) {
            $null = $psi.Environment.Remove($name)
        }
        $psi.UseShellExecute = $false
        $psi.RedirectStandardInput = $true
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
        $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8

        $process = [System.Diagnostics.Process]::Start($psi)
        $process.StandardInput.Close()
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $timedOut = -not $process.WaitForExit($TimeoutSec * 1000)
        if ($timedOut) {
            $process.Kill($true)
            $process.WaitForExit(2000) | Out-Null
            return [pscustomobject]@{
                ExitCode = -1
                TimedOut = $true
                Stdout = ""
                Stderr = ""
            }
        }
        $process.WaitForExit()
        return [pscustomobject]@{
            ExitCode = $process.ExitCode
            TimedOut = $false
            Stdout = $stdoutTask.Result
            Stderr = $stderrTask.Result
        }
    } finally {
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Ensure-TriageLabel([string]$Name, [System.Collections.Generic.HashSet[string]]$Ensured) {
    if ($Ensured.Contains($Name)) { return }
    $def = $script:TriageLabelByName[$Name]
    if ($null -eq $def) { throw "Unknown triage label '$Name'" }
    if ($Name -eq $script:TriageProtectedLabel) {
        throw "Refusing to create or modify protected label '$Name'"
    }
    & gh label create $def.Name -R $Repository --color $def.Color --description $def.Description --force | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "could not create or update label '$Name'" }
    $null = $Ensured.Add($Name)
}

$rubricPath = Join-Path $PSScriptRoot "TRIAGE_RUBRIC.md"
$rubricText = Get-Content $rubricPath -Raw

$openJson = & gh issue list -R $Repository --state open --limit 500 --json number,title,body,labels,state,url
if ($LASTEXITCODE -ne 0) { throw "could not list open issues in $Repository" }
$openIssues = @($openJson | ConvertFrom-Json)

$candidates = @()
if ($IssueNumber -gt 0) {
    $targetJson = & gh issue view $IssueNumber -R $Repository --json number,title,body,labels,state,url
    if ($LASTEXITCODE -ne 0) { throw "could not view issue #$IssueNumber in $Repository" }
    $targetIssue = $targetJson | ConvertFrom-Json
    if ([string]$targetIssue.state -ne "OPEN") {
        Write-Host "skip #${IssueNumber}: issue is closed; triager never modifies closed issues"
        exit 0
    }
    if (-not (Test-TriageEligibleIssue $targetIssue -Retriage:$Retriage)) {
        Write-Host "skip #${IssueNumber}: already has a priority label (use -Retriage to re-evaluate)"
        exit 0
    }
    $candidates = @($targetIssue)
} else {
    $candidates = @(
        $openIssues |
            Where-Object { Test-TriageEligibleIssue $_ -Retriage:$Retriage } |
            Sort-Object { [int]$_.number } |
            Select-Object -First $MaxIssues
    )
}

if (-not $candidates.Count) {
    Write-Host "triage: no eligible open issues found"
    exit 0
}

Write-Host "triage: $($candidates.Count) eligible open issue(s) selected$(if ($DryRun) { ' [DRY RUN]' })"
$shortcutsByNumber = @{}
foreach ($issue in $candidates) {
    $shortcut = Get-TriageShortcut $issue
    $shortcutsByNumber[[int]$issue.number] = $shortcut
    if ($shortcut.HasShortcut) {
        Write-Host "  shortcut #$($issue.number): $($shortcut.Reasons -join '; ')"
    }
}

$runner = Resolve-OpenCodeRunner
$exhausted = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
$ensuredLabels = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
$priorityCounts = @{ 1 = 0; 2 = 0; 3 = 0; 4 = 0; 5 = 0 }
$areaCounts = @{}
$triagedCount = 0
$skippedInvalid = 0
$rateLimitPattern = "(?i)(rate.?limit|too many requests|\b429\b|usage limit reached|free usage limit|insufficient (credit|quota)|quota exceeded)"

foreach ($issue in $candidates) {
    $number = [int]$issue.number
    $shortcut = $shortcutsByNumber[$number]
    $available = @($Models | Where-Object { -not $exhausted.Contains($_) })
    if (-not $available.Count) {
        Write-Host "triage: every free model is rate limited; stopping this round cleanly"
        break
    }

    $prompt = New-TriagePrompt $rubricText $issue $openIssues $shortcut
    $plan = $null

    foreach ($model in $available) {
        $result = Invoke-OpenCodeTriage $runner $model $prompt $TimeoutSeconds
        $combined = ($result.Stdout + "`n" + $result.Stderr) -replace "\x1b\[[0-9;]*[A-Za-z]", ""

        if ($combined -match $rateLimitPattern) {
            $null = $exhausted.Add($model)
            Write-Host "triage #${number}: $model reached its free limit; trying next model"
            continue
        }
        if ($result.TimedOut) {
            Write-Warning "triage #${number}: $model timed out after ${TimeoutSeconds}s; skipping issue"
            $skippedInvalid++
            break
        }
        if ($result.ExitCode -ne 0 -and [string]::IsNullOrWhiteSpace($result.Stdout)) {
            Write-Warning "triage #${number}: OpenCode exited $($result.ExitCode); skipping issue"
            $skippedInvalid++
            break
        }

        $parsed = ConvertFrom-TriageModelOutput $result.Stdout $number
        if (-not $parsed.Valid) {
            Write-Warning "triage #${number}: ignoring invalid model output ($($parsed.Error))"
            $skippedInvalid++
            break
        }

        $plan = Resolve-TriagePlan $issue $parsed $shortcut -Retriage:$Retriage
        break
    }

    if ($null -eq $plan) {
        if (@($Models | Where-Object { -not $exhausted.Contains($_) }).Count -eq 0) {
            Write-Host "triage: every free model is rate limited; stopping this round cleanly"
            break
        }
        continue
    }

    $priorityCounts[$plan.Priority]++
    $areaCounts[$plan.Area] = 1 + [int]($areaCounts[$plan.Area])
    $triagedCount++

    if ($DryRun) {
        $removeText = if ($plan.RemoveLabels.Count) { " remove [$($plan.RemoveLabels -join ', ')]" } else { "" }
        $preservedNote = if ($plan.PreservedInProgress) { " (preserving em-andamento)" } else { "" }
        Write-Host "DRY RUN #${number}: add [$($plan.AddLabels -join ', ')]$removeText$preservedNote | $($plan.Comment)"
        continue
    }

    # Re-check live state before writing so closed issues are never touched and
    # `em-andamento` is never removed.
    $live = & gh issue view $number -R $Repository --json state,labels | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "could not re-read issue #$number" }
    if ([string]$live.state -ne "OPEN") {
        Write-Host "skip #${number}: issue was closed during triage; skipping write"
        continue
    }

    $safeRemove = @($plan.RemoveLabels | Where-Object { $_ -ne $script:TriageProtectedLabel })
    foreach ($label in $plan.AddLabels) {
        Ensure-TriageLabel $label $ensuredLabels
    }

    if ($plan.AddLabels.Count -gt 0 -or $safeRemove.Count -gt 0) {
        $editArgs = @("issue", "edit", "$number", "-R", $Repository)
        foreach ($label in $plan.AddLabels) { $editArgs += @("--add-label", $label) }
        foreach ($label in $safeRemove) { $editArgs += @("--remove-label", $label) }
        & gh @editArgs | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "could not edit labels for issue #$number" }
    }

    & gh issue comment $number -R $Repository --body $plan.Comment | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "could not comment on issue #$number" }
    Write-Host "triaged #${number}: $($plan.PriorityLabel) $($plan.Area)$(if ($plan.DuplicateOf) { " duplicata? (#$($plan.DuplicateOf))" })"
}

Write-Host ""
Write-Host "triage summary: $triagedCount triaged, $skippedInvalid skipped (invalid/timeout), $($candidates.Count) candidate(s)"
Write-Host "priority distribution: P5=$($priorityCounts[5]), P4=$($priorityCounts[4]), P3=$($priorityCounts[3]), P2=$($priorityCounts[2]), P1=$($priorityCounts[1])"
if ($areaCounts.Count -gt 0) {
    $areaSummary = @($areaCounts.GetEnumerator() | Sort-Object Name | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join ", "
    Write-Host "area distribution: $areaSummary"
}
