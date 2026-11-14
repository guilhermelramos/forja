# Strict verdict rules shared by apply-verdicts.ps1 and its fixture test.
# A `fixed` verdict is only accepted when its evidence does not report a
# failure. Evidence that says the behavior did not work is downgraded to
# `still_broken`, even if the model wrote `fixed`.

$script:VerdictFailureSignals = @(
    'não teve efeito',
    'não funcionou',
    'não abriu',
    'falhou',
    'sem efeito',
    'did not',
    'no effect'
)

function ConvertTo-VerdictEvidenceKey([string]$Text) {
    if ([string]::IsNullOrWhiteSpace($Text)) { return '' }
    $normalized = $Text.Normalize([System.Text.NormalizationForm]::FormD)
    $normalized = [regex]::Replace($normalized, '\p{Mn}', '')
    return $normalized.ToLowerInvariant()
}

function Test-VerdictEvidenceFailure([object[]]$Steps, [string]$Actual) {
    $parts = @()
    foreach ($step in @($Steps)) {
        if ($null -ne $step) { $parts += [string]$step }
    }
    if ($Actual) { $parts += $Actual }
    $evidence = ConvertTo-VerdictEvidenceKey ($parts -join "`n")
    foreach ($signal in $script:VerdictFailureSignals) {
        if ($evidence.Contains((ConvertTo-VerdictEvidenceKey $signal))) { return $true }
    }
    return $false
}

function Get-EffectiveVerdict([string]$Verdict, [object[]]$Steps, [string]$Actual) {
    if ($Verdict -eq 'fixed' -and (Test-VerdictEvidenceFailure $Steps $Actual)) {
        return 'still_broken'
    }
    return $Verdict
}
