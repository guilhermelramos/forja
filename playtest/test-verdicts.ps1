# Fixture test for the strict verdict rule used by apply-verdicts.ps1.
# Run: ./playtest/test-verdicts.ps1

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "verdict-rules.ps1")

$fixturesPath = Join-Path $PSScriptRoot "fixtures/verdicts.jsonl"
$fixtures = @(Get-Content $fixturesPath | Where-Object { $_.Trim() } |
    ForEach-Object { $_ | ConvertFrom-Json })

$expected = @{
    901 = "still_broken"      # "não teve efeito"
    902 = "still_broken"      # "did not" / "no effect"
    903 = "fixed"             # clean evidence
    904 = "still_broken"      # "falhou"
    905 = "still_broken"      # already reported as broken
    906 = "cannot_reproduce"  # cannot set up the conditions
    907 = "still_broken"      # "não abriu"
    908 = "still_broken"      # "não funcionou"
    909 = "still_broken"      # "sem efeito"
}

if ($fixtures.Count -ne $expected.Count) {
    throw "fixture count: expected $($expected.Count), got $($fixtures.Count)"
}

foreach ($fixture in $fixtures) {
    $number = [int]$fixture.issue
    if (-not $expected.ContainsKey($number)) { throw "unexpected fixture issue #$number" }
    if ($fixture.title -ne "VEREDITO: $($fixture.verdict)") {
        throw "fixture #${number}: title does not match its verdict"
    }
    $effective = Get-EffectiveVerdict $fixture.verdict $fixture.steps $fixture.actual
    if ($effective -ne $expected[$number]) {
        throw "fixture #${number}: expected '$($expected[$number])', got '$effective'"
    }
    if ($number -in @(901, 902, 904, 907, 908, 909) -and $effective -eq $fixture.verdict) {
        throw "fixture #${number}: failure evidence was not downgraded"
    }
}

Write-Host "verdict fixtures: $($fixtures.Count) cases passed; failure evidence is downgraded to still_broken"
