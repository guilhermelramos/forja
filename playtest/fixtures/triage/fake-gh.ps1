# Offline `gh` used only by playtest/test-triage.ps1.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)

$ErrorActionPreference = "Stop"
if (-not $env:FORJA_TRIAGE_FIXTURES) { throw "FORJA_TRIAGE_FIXTURES is not set" }

$issues = @(Get-Content $env:FORJA_TRIAGE_FIXTURES -Raw | ConvertFrom-Json)

function Write-Json($Value) {
    ConvertTo-Json -InputObject $Value -Depth 20 -Compress
}

function Record-Write([string]$Command) {
    if (-not $env:FORJA_FAKE_GH_WRITES) {
        throw "fake gh: write '$Command' is not allowed"
    }
    Add-Content -Path $env:FORJA_FAKE_GH_WRITES -Value $Command -Encoding utf8
    Write-Output "ok"
}

if ($Arguments.Count -lt 1) { throw "fake gh: no arguments" }
switch ($Arguments[0]) {
    "issue" {
        if ($Arguments.Count -lt 2) { throw "fake gh: missing issue subcommand" }
        switch ($Arguments[1]) {
            "list" {
                $state = "open"
                for ($i = 2; $i -lt $Arguments.Count; $i++) {
                    if ($Arguments[$i] -eq "--state" -and $i + 1 -lt $Arguments.Count) {
                        $state = $Arguments[$i + 1]
                        $i++
                    }
                }
                $matching = if ($state -eq "all") {
                    $issues
                } else {
                    $want = if ($state -eq "open") { "OPEN" } else { "CLOSED" }
                    @($issues | Where-Object { [string]$_.state -eq $want })
                }
                Write-Json @($matching)
            }
            "view" {
                $number = [int]$Arguments[2]
                $issue = $issues | Where-Object { [int]$_.number -eq $number } | Select-Object -First 1
                if ($null -eq $issue) { throw "fake gh: issue #$number not found" }
                Write-Json $issue
            }
            "edit" { Record-Write ($Arguments -join " ") }
            "comment" { Record-Write ($Arguments -join " ") }
            "close" { throw "fake gh: close must NEVER be called by triage" }
            "reopen" { throw "fake gh: reopen must NEVER be called by triage" }
            default { throw "fake gh: unsupported issue command '$($Arguments[1])'" }
        }
    }
    "label" {
        if ($Arguments.Count -ge 3 -and $Arguments[1] -eq "create") {
            Record-Write "label create $($Arguments[2])"
        } else {
            throw "fake gh: unsupported label command"
        }
    }
    default { throw "fake gh: unsupported command '$($Arguments[0])'" }
}
