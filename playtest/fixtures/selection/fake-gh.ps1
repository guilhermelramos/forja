# Offline `gh` used only by playtest/test-selection.ps1. It reads the issue
# fixtures in FORJA_GH_FIXTURES and supports the read commands the verification
# scripts use. Writes are recorded in FORJA_FAKE_GH_WRITES when set; otherwise
# they are rejected. Tests run -DryRun unless they are asserting that a
# person-closed issue produces no write at all.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)

$ErrorActionPreference = "Stop"
if (-not $env:FORJA_GH_FIXTURES) { throw "FORJA_GH_FIXTURES is not set" }

$issues = @(Get-ChildItem -Path $env:FORJA_GH_FIXTURES -Filter "issue-*.json" |
    ForEach-Object { Get-Content $_.FullName -Raw | ConvertFrom-Json })

function Write-Json($Value) {
    ConvertTo-Json -InputObject $Value -Depth 40 -Compress
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
    "api" {
        $path = [string]$Arguments[1]
        if ($path -match "/issues/(\d+)/events") {
            $number = [int]$Matches[1]
            $issue = $issues | Where-Object { [int]$_.number -eq $number } | Select-Object -First 1
            if ($null -eq $issue) { throw "fake gh: issue #$number not found for events" }
            Write-Json @($issue.events)
        } else {
            # gh api repos/OWNER/REPO --jq .owner.login
            Write-Output "guilhermelramos"
        }
    }
    "issue" {
        if ($Arguments.Count -lt 2) { throw "fake gh: issue command is missing" }
        switch ($Arguments[1]) {
            "list" {
                $state = "open"
                for ($index = 2; $index -lt $Arguments.Count; $index++) {
                    if ($Arguments[$index] -eq "--state" -and $index + 1 -lt $Arguments.Count) {
                        $state = $Arguments[$index + 1]
                        $index++
                    }
                }
                $matching = if ($state -eq "all") {
                    $issues
                } else {
                    @($issues | Where-Object {
                        [string]$_.state -eq $(if ($state -eq "open") { "OPEN" } else { "CLOSED" })
                    })
                }
                $listed = @($matching | ForEach-Object {
                    [pscustomobject]@{
                        number = $_.number
                        title = $_.title
                        createdAt = $_.createdAt
                        labels = @($_.labels)
                        url = $_.url
                    }
                })
                Write-Json @($listed)
            }
            "view" {
                $number = [int]$Arguments[2]
                $issue = $issues | Where-Object { [int]$_.number -eq $number } | Select-Object -First 1
                if ($null -eq $issue) { throw "fake gh: issue #$number not found" }
                Write-Json $issue
            }
            "comment" { Record-Write "comment $($Arguments[2])" }
            "close" { Record-Write "close $($Arguments[2])" }
            "reopen" { Record-Write "reopen $($Arguments[2])" }
            "edit" { Record-Write "edit $($Arguments[2])" }
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
