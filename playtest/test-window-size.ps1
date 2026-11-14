# Fixture test for window-size parsing used by verify.ps1 and run.ps1.
# Run: ./playtest/test-window-size.ps1

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "window-size.ps1")

$cases = @(
    @{ text = "Janela padrão 1280×800"; expected = "1280x800" },
    @{ text = "Sem maximizar (janela 1280x800)"; expected = "1280x800" },
    @{ text = "tela em 1920 X 1080"; expected = "1920x1080" },
    @{ text = "1280 × 800"; expected = "1280x800" },
    @{ text = "maximizando para 1920×1032 o layout fica usável"; expected = "1920x1032" },
    @{ text = "sem tamanho citado"; expected = $null },
    @{ text = "1280"; expected = $null },
    @{ text = "1280*800"; expected = $null }
)

foreach ($case in $cases) {
    $actual = ConvertTo-ForjaWindowSize $case.text
    if ($actual -ne $case.expected) {
        throw "window size from '$($case.text)': expected '$($case.expected)', got '$actual'"
    }
}

if ((Get-ForjaWindowSizeFromTexts @("sem tamanho", "outra 1280×800")) -ne "1280x800") {
    throw "window size should be found in later texts"
}
if ((Test-ForjaWindowSizeValue " 1280×800 ") -ne "1280x800") {
    throw "WindowSize override should normalize the multiplication sign"
}
if ($null -ne (Test-ForjaWindowSizeValue "")) {
    throw "empty WindowSize override should use the app default"
}
try {
    $null = Test-ForjaWindowSizeValue "1280*800"
    throw "invalid WindowSize was accepted"
} catch {
    if ($_.Exception.Message -notmatch "WIDTHxHEIGHT") { throw }
}

Write-Host "window size fixtures: $($cases.Count) cases passed; issue citations are detected"
