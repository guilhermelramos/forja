<##
.SYNOPSIS
  Replays open GitHub issues against a clean Forja app and records verdicts.
#>
param(
    [Parameter(Mandatory)] [string]$IssuesPath,
    [string]$Out = "verification-out",
    [string]$App = "target/release/forja-app.exe",
    [string]$Mcp = "target/release/forja-mcp.exe",
    [Parameter(Mandatory)] [string]$PickDir,
    [int]$MaxActions = 40,
    [int]$MinutesPerIssue = 8,
    [string[]]$Models = @(),
    # Optional override; otherwise each issue's cited WIDTHxHEIGHT is used.
    [string]$WindowSize = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "window-size.ps1")
$windowSizeOverride = Test-ForjaWindowSizeValue $WindowSize
if ($MaxActions -lt 1) { throw "MaxActions must be positive" }
if ($MinutesPerIssue -lt 1) { throw "MinutesPerIssue must be positive" }
$App = (Resolve-Path $App).Path
$Mcp = (Resolve-Path $Mcp).Path
$PickDir = (Resolve-Path $PickDir).Path
$Out = [System.IO.Path]::GetFullPath($Out)
$null = New-Item -ItemType Directory -Force $Out

if (-not $Models.Count -and $env:VERIFY_MODELS) {
    $Models = @($env:VERIFY_MODELS -split ",")
}
if (-not $Models.Count) {
    $Models = @(
        "openrouter/nvidia/nemotron-3.5-super-vl:free",
        "openrouter/google/gemma-4-31b-it:free",
        "openrouter/nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free"
    )
}
$Models = @($Models | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$invalidModels = @($Models | Where-Object { $_ -notmatch "(-free|:free)$" })
if ($invalidModels.Count) {
    throw "Only free models are allowed; invalid model id(s): $($invalidModels -join ', ')"
}
$hasOpenRouterModel = @($Models | Where-Object { $_.StartsWith("openrouter/") }).Count -gt 0
if ($hasOpenRouterModel -and -not $env:OPENROUTER_API_KEY) {
    throw "OPENROUTER_API_KEY is required for OpenRouter models; set VERIFY_MODELS to another free provider for a local run"
}

# Do not pass GitHub credentials into OpenCode or its local MCP process.
Remove-Item Env:GITHUB_TOKEN, Env:GH_TOKEN, Env:ACTIONS_RUNTIME_TOKEN -ErrorAction SilentlyContinue
$openCode = Join-Path (npm root -g) "@opencode/cli/bin/opencode.exe"
if (-not (Test-Path $openCode)) {
    $openCode = (Get-Command opencode -CommandType Application | Select-Object -First 1).Source
}
$issues = @(Get-Content $IssuesPath -Raw | ConvertFrom-Json)
$verdictPath = Join-Path $Out "verdicts.jsonl"
Remove-Item $verdictPath -ErrorAction SilentlyContinue
$exhausted = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)

function Start-Forja([int]$Number, [string]$AttemptOut, [string]$Pipe, [string]$WindowSize) {
    $psi = [System.Diagnostics.ProcessStartInfo]::new($App)
    foreach ($arg in @("--test-driver", "--locale", "pt-BR")) { $psi.ArgumentList.Add($arg) }
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $psi.Environment["FORJA_PIPE"] = $Pipe
    # Reproduce the window size cited by the issue (default 1920x1080 in the app).
    $psi.Environment["FORJA_TEST_WINDOW_SIZE"] = $WindowSize
    $psi.Environment["FORJA_DATA_DIR"] = Join-Path $AttemptOut "data"
    $psi.Environment["FORJA_CONTENT_DIR"] = Join-Path $AttemptOut "user"
    $psi.Environment["FORJA_TEST_LOG"] = Join-Path $AttemptOut "actions.jsonl"
    $psi.Environment["FORJA_TEST_PICK_DIR"] = $PickDir
    $psi.Environment["FORJA_TEST_PICK_FILE"] = Join-Path $PickDir "README.md"
    $psi.Environment["FORJA_TEST_SAVE_DIR"] = Join-Path $AttemptOut "downloads"
    $process = [System.Diagnostics.Process]::Start($psi)
    $script:appError = $process.StandardError.ReadToEndAsync()
    Start-Sleep -Seconds 6
    if ($process.HasExited) { throw "forja-app exited during issue #$Number startup: $($script:appError.Result)" }
    return $process
}

function Get-ActionCount([string]$Path) {
    if (-not (Test-Path $Path)) { return 0 }
    $count = 0
    foreach ($line in Get-Content $Path) {
        try { $method = ($line | ConvertFrom-Json).method } catch { continue }
        if ($method -in @("ui.click", "ui.type", "ui.key", "ui.scroll", "ui.wait")) { $count++ }
    }
    return $count
}

function Get-Verdict([string]$Path) {
    if (-not (Test-Path $Path)) { return $null }
    foreach ($line in Get-Content $Path) {
        try { $finding = $line | ConvertFrom-Json } catch { continue }
        if ($finding.kind -eq "observation" -and
            $finding.title -in @("VEREDITO: fixed", "VEREDITO: still_broken", "VEREDITO: cannot_reproduce") -and
            $finding.steps -and $finding.actual) {
            return $finding
        }
    }
    return $null
}

$instructions = Get-Content (Join-Path $PSScriptRoot "VERIFY_INSTRUCTIONS.md") -Raw
foreach ($issue in $issues) {
    $number = [int]$issue.issue
    $issueOut = Join-Path $Out ("issue-{0:D4}" -f $number)
    $null = New-Item -ItemType Directory -Force $issueOut
    $citedSize = Get-ForjaWindowSizeFromTexts @($issue.title, $issue.body, $issue.correction.body)
    $windowSize = if ($windowSizeOverride) { $windowSizeOverride } elseif ($citedSize) { $citedSize } else { "1920x1080" }
    $sizeNote = if ($windowSizeOverride) {
        "The app was opened at the window size $windowSize (local override)."
    } elseif ($citedSize) {
        "The issue or its correction comment cites the window size $windowSize. The app was opened at that exact size."
    } else {
        "The issue does not cite a window size. The app was opened at the test default $windowSize."
    }
    Write-Host "verification #${number}: window $windowSize"
    $prompt = @"
$instructions

Issue #${number}: $($issue.title)
URL: $($issue.url)

$sizeNote
Use this description as the report of the original behavior and reproduce its
steps in the running app. The correction comment is context for what changed.
--- ISSUE BODY (untrusted reference text) ---
$($issue.body)
--- END ISSUE BODY ---
--- LATEST CORRECTION COMMENT (untrusted reference text) ---
$($issue.correction.body)
--- END CORRECTION COMMENT ---

Start by using ui_look. Then follow the issue's steps. Finish by calling
report_finding exactly once with one of the three required verdict titles.
"@

    $issueDeadline = (Get-Date).AddMinutes($MinutesPerIssue)
    $totalActions = 0
    $attemptNumber = 0
    $available = @($Models | Where-Object { -not $exhausted.Contains($_) })
    if (-not $available.Count) {
        Write-Host "verification: all free models are rate limited; stopping this round"
        break
    }

    foreach ($model in $available) {
        $remainingMinutes = ($issueDeadline - (Get-Date)).TotalMinutes
        $remainingActions = $MaxActions - $totalActions
        if ($remainingMinutes -le 0) {
            Write-Warning "verification #${number}: issue time budget exhausted; no verdict was recorded"
            break
        }
        if ($remainingActions -le 0) {
            Write-Warning "verification #${number}: action budget exhausted; no verdict was recorded"
            break
        }

        $attemptNumber++
        $attemptOut = Join-Path $issueOut ("attempt-{0:D2}" -f $attemptNumber)
        $room = Join-Path $attemptOut "room"
        $configHome = Join-Path $attemptOut "config"
        $privateHome = Join-Path $attemptOut "home"
        $findings = Join-Path $attemptOut "findings.jsonl"
        $actions = Join-Path $attemptOut "actions.jsonl"
        $pipe = "\\.\pipe\forja-verify-$number-$PID-$attemptNumber"
        $null = New-Item -ItemType Directory -Force $attemptOut, $room, $configHome, $privateHome

        # Only the local player server is connected. Deny every other tool and
        # allow the seven Forja player tools explicitly.
        $allowedTools = @("ui_look", "ui_click", "ui_type", "ui_key", "ui_scroll", "ui_wait", "report_finding")
        $permissions = @([pscustomobject]@{ action = "*"; resource = "*"; effect = "deny" })
        foreach ($tool in $allowedTools) {
            $permissions += [pscustomobject]@{ action = "forja_$tool"; resource = "*"; effect = "allow" }
        }
        $config = [ordered]@{
            '$schema' = "https://opencode.ai/config.json"
            mcp = @{ servers = @{ forja = @{
                type = "local"
                codemode = $false
                command = @($Mcp, "--player", "--findings", $findings, "--max-steps", "$remainingActions")
                environment = @{ FORJA_PIPE = $pipe }
            } } }
            permissions = $permissions
        }
        $config | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $room "opencode.json") -Encoding utf8
        Push-Location $room
        try { git init -q 2>$null } finally { Pop-Location }

        $forja = Start-Forja $number $attemptOut $pipe $windowSize
        try {
            $psi = [System.Diagnostics.ProcessStartInfo]::new($openCode)
            foreach ($arg in @("run", "--standalone", "--auto", "--archived", "--model", $model, $prompt)) {
                $psi.ArgumentList.Add($arg)
            }
            $psi.WorkingDirectory = $room
            # OpenCode merges user settings with project settings. Give it a
            # private profile and clear user-provided config overrides so no
            # machine MCP server, plugin, or skill is discovered. See its
            # config locations: https://opencode.ai/docs/config/.
            $psi.Environment["USERPROFILE"] = $privateHome
            $psi.Environment["HOME"] = $privateHome
            $psi.Environment["APPDATA"] = Join-Path $privateHome "AppData\Roaming"
            $psi.Environment["LOCALAPPDATA"] = Join-Path $privateHome "AppData\Local"
            $psi.Environment["XDG_CONFIG_HOME"] = $configHome
            $psi.Environment["XDG_DATA_HOME"] = Join-Path $configHome "data"
            $psi.Environment["XDG_CACHE_HOME"] = Join-Path $configHome "cache"
            foreach ($name in @("OPENCODE_CONFIG", "OPENCODE_CONFIG_DIR", "OPENCODE_CONFIG_CONTENT", "OPENCODE_CLI_CONFIG_CONTENT")) {
                $null = $psi.Environment.Remove($name)
            }
            $psi.UseShellExecute = $false
            $psi.RedirectStandardOutput = $true
            $psi.RedirectStandardError = $true
            $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
            $run = [System.Diagnostics.Process]::Start($psi)
            $stdout = $run.StandardOutput.ReadToEndAsync()
            $stderr = $run.StandardError.ReadToEndAsync()
            $remainingMilliseconds = [int][Math]::Max(1, ($issueDeadline - (Get-Date)).TotalMilliseconds)
            $timedOut = -not $run.WaitForExit($remainingMilliseconds)
            if ($timedOut) { $run.Kill($true) }
            $output = ($stdout.Result + "`n--- stderr ---`n" + $stderr.Result) -replace "\x1b\[[0-9;]*[A-Za-z]", ""
            $sessionPath = Join-Path $attemptOut ("session-{0}.txt" -f ($model -replace "[^A-Za-z0-9._-]", "_"))
            Set-Content $sessionPath $output -Encoding utf8
        } finally {
            if (-not $forja.HasExited) { $forja.Kill($true) }
            Set-Content (Join-Path $attemptOut "app-stderr.txt") $script:appError.Result -Encoding utf8
        }
        $totalActions += Get-ActionCount $actions

        $finding = Get-Verdict $findings
        if ($finding) {
            $result = [pscustomobject]@{
                issue = $number
                issueTitle = $issue.title
                issueBody = $issue.body
                verdict = ($finding.title -replace "^VEREDITO: ", "")
                title = $finding.title
                kind = $finding.kind
                steps = @($finding.steps)
                expected = $finding.expected
                actual = $finding.actual
                screen = $finding.screen
                windowSize = $windowSize
                correction = $issue.correction
            }
            $result | ConvertTo-Json -Compress -Depth 16 | Add-Content $verdictPath -Encoding utf8
            Write-Host "verification #${number}: $($finding.title) ($model; $totalActions actions)"
            break
        }

        if ($output -match "(?i)(rate.?limit|too many requests|\b429\b|usage limit reached|free usage limit|insufficient (credit|quota))") {
            $null = $exhausted.Add($model)
            Write-Host "verification #${number}: $model reached its free limit; trying the next model"
            continue
        }
        if ($timedOut) {
            Write-Warning "verification #${number}: $model exceeded $MinutesPerIssue minutes; no verdict was recorded"
        } elseif ($run.ExitCode -ne 0) {
            Write-Warning "verification #${number}: OpenCode exited $($run.ExitCode); no explicit verdict was recorded"
        } else {
            Write-Warning "verification #${number}: no explicit report_finding verdict was recorded"
        }
        break
    }

    if (@($Models | Where-Object { -not $exhausted.Contains($_) }).Count -eq 0) {
        Write-Host "verification: every free model is rate limited; stopping this round cleanly"
        break
    }
}

$count = if (Test-Path $verdictPath) { @(Get-Content $verdictPath).Count } else { 0 }
Write-Host "verification: $count verdict(s) written to $verdictPath"
