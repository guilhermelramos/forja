# Offline `gh` used only by playtest/test-selection.ps1. It reads the issue
# fixtures in FORJA_GH_FIXTURES and supports the read commands the verification
# scripts use. Writes are rejected on purpose: tests run with -DryRun.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)

$ErrorActionPreference = "Stop"
if (-not $env:FORJA_GH_FIXTURES) { throw "FORJA_GH_FIXTURES is not set" }

$issues = @(Get-ChildItem -Path $env:FORJA_GH_FIXTURES -Filter "issue-*.json" |
    ForEach-Object { Get-Content $_.FullName -Raw | ConvertFrom-Json })

function Write-Json($Value) {
    ConvertTo-Json -InputObject $Value -Depth 30 -Compress
}

if ($Arguments.Count -lt 1) { throw "fake gh: no arguments" }
switch ($Arguments[0]) {
    "api" {
        # gh api repos/OWNER/REPO --jq .owner.login
        Write-Output "guilhermelramos"
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
            default { throw "fake gh: unsupported issue command '$($Arguments[1])'" }
        }
    }
    default { throw "fake gh: unsupported command '$($Arguments[0])'" }
}
