# Shared window-size parsing for the play-test scripts.
# `FORJA_TEST_WINDOW_SIZE` accepts WIDTHxHEIGHT; issues often write `1280×800`.

function ConvertTo-ForjaWindowSize([string]$Text) {
    if ([string]::IsNullOrWhiteSpace($Text)) { return $null }
    $match = [regex]::Match($Text, '(?<!\d)(\d{3,4})\s*[x×X]\s*(\d{3,4})(?!\d)')
    if (-not $match.Success) { return $null }
    $width = [int]$match.Groups[1].Value
    $height = [int]$match.Groups[2].Value
    if ($width -lt 1 -or $height -lt 1) { return $null }
    return "${width}x${height}"
}

function Get-ForjaWindowSizeFromTexts([string[]]$Texts) {
    foreach ($text in $Texts) {
        $size = ConvertTo-ForjaWindowSize $text
        if ($size) { return $size }
    }
    return $null
}

function Test-ForjaWindowSizeValue([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return $null }
    $normalized = $Value.Trim() -replace '[×X]', 'x'
    if ($normalized -notmatch '^\d{3,4}x\d{3,4}$') {
        throw "WindowSize must be WIDTHxHEIGHT, for example 1280x800"
    }
    return $normalized
}
