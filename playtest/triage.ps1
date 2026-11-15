<#
.SYNOPSIS
  Automatic issue triager: assigns priority (1-5) and area labels to open issues.

.DESCRIPTION
  Selects open issues without a `prioridade-N` label (plus open issues marked
  `triagem-provisória`, or all open issues when `-Retriage` is set), evaluates
  deterministic pre-AI shortcuts, asks a free OpenCode model to classify
  priority, area, rationale, and possible duplicate, validates the JSON
  response, and applies labels and a single-line comment (or shows the plan
  with `-DryRun`). When no model responds (rate limit, timeout, or error on
  every model), applies a deterministic provisional priority and area with
  `triagem-provisória` so no issue is left untriaged; a later run with an
  available model refines provisional issues and removes `triagem-provisória`.
  Never closes issues, never touches closed issues, and never adds or removes
  `em-andamento`.
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
. (Join-Path $PSScriptRoot "model-rules.ps1")

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
        "openrouter/z-ai/glm-5.3-flash",
        "openrouter/nvidia/nemotron-3.5-lightning:free",
        "openrouter/nvidia/nemotron-3-ultra-550b-a55b:free",
        "openrouter/google/gemma-4-31b-it:free"
    )
}
$Models = Assert-AllowedPlaytestModels $Models
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

function Resolve-OpenRouterSdkClassifier {
    $nodeCmd = Get-Command node -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $nodeCmd) {
        throw "Node.js is required for OpenRouter SDK triage"
    }
    $tsDir = Join-Path $PSScriptRoot "ts"
    $tsSource = Join-Path $tsDir "triage-classify.ts"
    $jsBuilt = Join-Path $tsDir "dist\triage-classify.js"
    $nodeModules = Join-Path $tsDir "node_modules"
    if (-not (Test-Path $nodeModules)) {
        throw "OpenRouter SDK dependencies are missing in $tsDir; run 'npm ci' inside playtest/ts"
    }
    $needsBuild = (-not (Test-Path $jsBuilt)) -or
        ((Get-Item $tsSource).LastWriteTimeUtc -gt (Get-Item $jsBuilt).LastWriteTimeUtc)
    if ($needsBuild) {
        $npmCmd = Get-Command npm -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($null -eq $npmCmd) {
            throw "npm is required to build playtest/ts/triage-classify.ts"
        }
        Push-Location $tsDir
        try {
            & $npmCmd.Source run build | Out-Null
            if ($LASTEXITCODE -ne 0 -or -not (Test-Path $jsBuilt)) {
                throw "Failed to build playtest/ts/triage-classify.ts"
            }
        } finally {
            Pop-Location
        }
    }
    return [pscustomobject]@{
        NodeExecutable = $nodeCmd.Source
        ScriptPath = $jsBuilt
    }
}

function Invoke-OpenRouterSdkTriage($Classifier, [string]$Model, [string]$Prompt, [int]$TimeoutSec) {
    $psi = [System.Diagnostics.ProcessStartInfo]::new($Classifier.NodeExecutable)
    foreach ($arg in @($Classifier.ScriptPath, "--model", $Model, "--timeout-ms", ([string]($TimeoutSec * 1000)))) {
        $psi.ArgumentList.Add($arg)
    }
    $psi.WorkingDirectory = Join-Path $PSScriptRoot "ts"
    foreach ($name in @("GITHUB_TOKEN", "GH_TOKEN", "ACTIONS_RUNTIME_TOKEN")) {
        $null = $psi.Environment.Remove($name)
    }
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardInputEncoding = [System.Text.Encoding]::UTF8
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8

    $process = [System.Diagnostics.Process]::Start($psi)
    $process.StandardInput.Write($Prompt)
    $process.StandardInput.Close()
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $timedOut = -not $process.WaitForExit(($TimeoutSec + 5) * 1000)
    if ($timedOut) {
        $process.Kill($true)
        $process.WaitForExit(2000) | Out-Null
        return [pscustomobject]@{
            ExitCode = -1
            TimedOut = $true
            Stdout = ""
            Stderr = ""
            Usage = $null
        }
    }
    $process.WaitForExit()
    $rawOut = $stdoutTask.Result
    $rawErr = $stderrTask.Result
    if ($process.ExitCode -eq 2) {
        return [pscustomobject]@{
            ExitCode = 2
            TimedOut = $true
            Stdout = ""
            Stderr = $rawErr
            Usage = $null
        }
    }

    $content = $rawOut
    $usage = $null
    if ($process.ExitCode -eq 0 -and -not [string]::IsNullOrWhiteSpace($rawOut)) {
        try {
            $envelope = $rawOut | ConvertFrom-Json -ErrorAction Stop
            if ($null -ne $envelope.PSObject.Properties["content"]) {
                $content = [string]$envelope.content
            }
            if ($null -ne $envelope.PSObject.Properties["usage"]) {
                $usage = $envelope.usage
            }
        } catch {
            # Let caller validate $content if envelope JSON is malformed.
        }
    }

    return [pscustomobject]@{
        ExitCode = $process.ExitCode
        TimedOut = $false
        Stdout = $content
        Stderr = $rawErr
        Usage = $usage
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

$useFakeOpenCode = [bool]$env:FORJA_OPENCODE_SCRIPT
$openRouterClassifier = $null
if (-not $useFakeOpenCode -and $hasOpenRouterModel) {
    $openRouterClassifier = Resolve-OpenRouterSdkClassifier
}
$hasNonOpenRouterModel = @($Models | Where-Object { -not $_.StartsWith("openrouter/") }).Count -gt 0
$runner = if ($useFakeOpenCode -or $hasNonOpenRouterModel) { Resolve-OpenCodeRunner } else { $null }
$exhausted = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
$consecutiveTimeouts = @{}
$consecutiveFailures = @{}
$ensuredLabels = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
$priorityCounts = @{ 1 = 0; 2 = 0; 3 = 0; 4 = 0; 5 = 0 }
$areaCounts = @{}
$triagedCount = 0
$provisionalCount = 0
$skippedInvalid = 0
$skippedExistingProvisional = 0
$loggedAllExhausted = $false
$totalPromptTokens = [long]0
$totalCompletionTokens = [long]0
$totalReasoningTokens = [long]0
$usageSamples = 0
$rateLimitPattern = "(?i)(rate.?limit|too many requests|\b429\b|usage limit reached|free usage limit|insufficient (credit|quota)|quota exceeded|no allowed providers|no endpoints found)"

foreach ($issue in $candidates) {
    $number = [int]$issue.number
    $shortcut = $shortcutsByNumber[$number]
    $available = @($Models | Where-Object { -not $exhausted.Contains($_) })
    $plan = $null
    $modelResponded = $false

    if (-not $available.Count) {
        if (-not $loggedAllExhausted) {
            Write-Host "triage: every model is unavailable (rate limited/timed out); applying deterministic provisional triage"
            $loggedAllExhausted = $true
        }
    } else {
        $prompt = New-TriagePrompt $rubricText $issue $openIssues $shortcut

        foreach ($model in $available) {
            $result = if (-not $useFakeOpenCode -and $model.StartsWith("openrouter/")) {
                Invoke-OpenRouterSdkTriage $openRouterClassifier $model $prompt $TimeoutSeconds
            } else {
                Invoke-OpenCodeTriage $runner $model $prompt $TimeoutSeconds
            }
            $combined = ($result.Stdout + "`n" + $result.Stderr) -replace "\x1b\[[0-9;]*[A-Za-z]", ""

            if ($null -ne $result.PSObject.Properties["Usage"] -and $null -ne $result.Usage) {
                $inTok = [long]($result.Usage.promptTokens ?? 0)
                $outTok = [long]($result.Usage.completionTokens ?? 0)
                $reasonTok = [long]($result.Usage.reasoningTokens ?? 0)
                $totalPromptTokens += $inTok
                $totalCompletionTokens += $outTok
                $totalReasoningTokens += $reasonTok
                $usageSamples++
                Write-Host "  usage #${number} ($model): input=$inTok, output=$outTok, reasoning=$reasonTok"
            }

            if ($combined -match $rateLimitPattern) {
                $null = $exhausted.Add($model)
                Write-Host "triage #${number}: $model unavailable or rate-limited; trying next model"
                continue
            }
            if ($result.TimedOut) {
                $consecutiveTimeouts[$model] = 1 + [int]($consecutiveTimeouts[$model])
                if ($consecutiveTimeouts[$model] -ge 2) {
                    $null = $exhausted.Add($model)
                }
                Write-Warning "triage #${number}: $model timed out after ${TimeoutSeconds}s; trying next model"
                continue
            }
            if ($result.ExitCode -ne 0) {
                $consecutiveFailures[$model] = 1 + [int]($consecutiveFailures[$model])
                if ($consecutiveFailures[$model] -ge 3) {
                    $null = $exhausted.Add($model)
                }
                Write-Warning "triage #${number}: $model failed (exit $($result.ExitCode)); trying next model"
                continue
            }
            if ([string]::IsNullOrWhiteSpace($result.Stdout)) {
                Write-Warning "triage #${number}: $model returned empty output; trying next model"
                continue
            }

            $consecutiveTimeouts[$model] = 0
            $consecutiveFailures[$model] = 0
            $modelResponded = $true

            $parsed = ConvertFrom-TriageModelOutput $result.Stdout $number
            if (-not $parsed.Valid) {
                Write-Warning "triage #${number}: ignoring invalid model output ($($parsed.Error))"
                $skippedInvalid++
                break
            }

            $plan = Resolve-TriagePlan $issue $parsed $shortcut -Retriage:$Retriage
            break
        }
    }

    if ($null -eq $plan) {
        if ($modelResponded) {
            continue
        }
        $existingLabels = @(Get-IssueLabelNames $issue)
        $alreadyProvisional = ($existingLabels -contains $script:TriageProvisionalLabel) -and
            (@($existingLabels | Where-Object { $_ -match $script:TriagePriorityPattern }).Count -gt 0)
        if ($alreadyProvisional -and -not $Retriage) {
            Write-Host "skip #${number}: already has $script:TriageProvisionalLabel and no model responded"
            $skippedExistingProvisional++
            continue
        }
        $plan = Resolve-ProvisionalTriagePlan $issue $shortcut -Retriage:$Retriage
        $provisionalCount++
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
    $provTag = if ($plan.IsProvisional) { " $script:TriageProvisionalLabel" } else { "" }
    $dupTag = if ($plan.DuplicateOf) { " duplicata? (#$($plan.DuplicateOf))" } else { "" }
    Write-Host "triaged #${number}: $($plan.PriorityLabel) $($plan.Area)$provTag$dupTag"
}

Write-Host ""
Write-Host "triage summary: $triagedCount triaged ($provisionalCount provisional), $skippedInvalid skipped (invalid), $skippedExistingProvisional kept provisional, $($candidates.Count) candidate(s)"
Write-Host "priority distribution: P5=$($priorityCounts[5]), P4=$($priorityCounts[4]), P3=$($priorityCounts[3]), P2=$($priorityCounts[2]), P1=$($priorityCounts[1])"
if ($areaCounts.Count -gt 0) {
    $areaSummary = @($areaCounts.GetEnumerator() | Sort-Object Name | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join ", "
    Write-Host "area distribution: $areaSummary"
}
if ($usageSamples -gt 0) {
    Write-Host "token usage summary: input=$totalPromptTokens, output=$totalCompletionTokens, reasoning=$totalReasoningTokens ($usageSamples call(s))"
}
