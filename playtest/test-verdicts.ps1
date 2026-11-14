# Fixture test for the structured verdict rule used by apply-verdicts.ps1.
# Run: ./playtest/test-verdicts.ps1

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "verdict-rules.ps1")

$fixturesPath = Join-Path $PSScriptRoot "fixtures/verdicts.jsonl"
$fixtures = @(Get-Content $fixturesPath | Where-Object { $_.Trim() } |
    ForEach-Object { $_ | ConvertFrom-Json })

$expected = @{
    901 = "still_broken"      # expectation ok=false
    902 = "still_broken"      # expectation ok=false (English)
    903 = "fixed"             # all expectations ok
    904 = "still_broken"      # expectation ok=false
    905 = "still_broken"      # already reported as broken
    906 = "cannot_reproduce"  # cannot set up the conditions
    907 = "still_broken"      # expectation ok=false
    908 = "still_broken"      # expectation ok=false
    909 = "still_broken"      # expectation ok=false
    910 = "fixed"             # real #64 evidence: "não abriu Ajustes" is the expected behavior
    911 = "fixed"             # structured expectations win over failure words
    912 = "fixed"             # legacy verdict without expectations is not downgraded
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
    $effective = Get-EffectiveVerdict $fixture.verdict $fixture.expectations
    if ($effective -ne $expected[$number]) {
        throw "fixture #${number}: expected '$($expected[$number])', got '$effective'"
    }
}

foreach ($number in @(901, 902, 904, 907, 908, 909)) {
    $fixture = $fixtures | Where-Object { $_.issue -eq $number }
    if ((Get-EffectiveVerdict "fixed" $fixture.expectations) -ne "still_broken") {
        throw "fixture #${number}: false expectation was not downgraded"
    }
}

$case64 = $fixtures | Where-Object { $_.issue -eq 910 }
if (@($case64.expectations | Where-Object { -not $_.ok }).Count -ne 0) {
    throw "fixture #910: every expectation of the real #64 case should be ok"
}
if (-not $case64.actual.Contains("Não abriu nenhuma janela de Ajustes")) {
    throw "fixture #910: the false-negative phrase is missing"
}
if ((Get-EffectiveVerdict $case64.verdict $case64.expectations) -ne "fixed") {
    throw "fixture #910: the real #64 case was downgraded again"
}

$legacy = $fixtures | Where-Object { $_.issue -eq 912 }
if ($null -ne $legacy.expectations -or (Get-EffectiveVerdict $legacy.verdict $legacy.expectations) -ne "fixed") {
    throw "fixture #912: a legacy verdict without expectations must not be downgraded"
}

Write-Host "verdict fixtures: $($fixtures.Count) cases passed; structured expectations decide fixed, legacy failure words only warn"
