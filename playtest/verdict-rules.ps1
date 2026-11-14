# Shared rules for selecting issues awaiting verification and applying verdicts.
#
# A `fixed` verdict is decided by the structured `expectations` list emitted by
# the verifier: every item must say `ok: true`. The word heuristic below is only
# a log hint for legacy verdicts without a list; it never downgrades a verdict.

$script:CorrectionPrefixes = @(
    'Corrigido na main',
    'Tratado na main',
    'Concluído na main'
)

$script:VerdictPrefix = 'Verificado:'

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

# Returns the valid `{ item, ok, observed }` entries, keeping only complete
# entries. An absent or legacy value returns an empty list.
function Get-VerificationExpectations([object]$Expectations) {
    if ($null -eq $Expectations) { return @() }
    $list = [System.Collections.Generic.List[object]]::new()
    foreach ($entry in @($Expectations)) {
        if ($null -eq $entry) { continue }
        $item = ([string]$entry.item).Trim()
        $observed = ([string]$entry.observed).Trim()
        if ([string]::IsNullOrWhiteSpace($item) -or [string]::IsNullOrWhiteSpace($observed)) {
            continue
        }
        $okProperty = $entry.PSObject.Properties['ok']
        if ($null -eq $okProperty -or $okProperty.Value -isnot [bool]) { continue }
        $list.Add([pscustomobject]@{
            item = $item
            ok = [bool]$okProperty.Value
            observed = $observed
        })
    }
    return $list.ToArray()
}

function Get-EffectiveVerdict([string]$Verdict, [object]$Expectations) {
    if ($Verdict -ne 'fixed') { return $Verdict }
    $items = @(Get-VerificationExpectations $Expectations)
    if (-not $items.Count) { return $Verdict }
    if (@($items | Where-Object { -not $_.ok }).Count -gt 0) {
        return 'still_broken'
    }
    return $Verdict
}

function Get-SortedComments([object]$Issue) {
    return @($Issue.comments | Sort-Object { [DateTimeOffset]$_.createdAt })
}

function Find-LatestCorrectionComment([object[]]$Comments, [string]$Owner) {
    $matches = @($Comments | Where-Object {
        $body = [string]$_.body
        $_.author -and $_.author.login -eq $Owner -and
        @($script:CorrectionPrefixes | Where-Object { $body.StartsWith($_) }).Count -gt 0
    })
    if (-not $matches.Count) { return $null }
    return $matches[-1]
}

function Find-LatestVerdictComment([object[]]$Comments) {
    $matches = @($Comments | Where-Object {
        ([string]$_.body).StartsWith($script:VerdictPrefix)
    })
    if (-not $matches.Count) { return $null }
    return $matches[-1]
}

# An issue awaits verification when its newest owner correction is newer than
# its newest verdict. Closed issues are included only when the correction was
# posted after the close: that is the coordinator asking for a re-check after a
# previous verdict closed the issue.
function Get-PendingVerification([object]$Issue, [string]$Owner) {
    if ($null -eq $Issue) { return $null }
    $comments = @(Get-SortedComments $Issue)
    $correction = Find-LatestCorrectionComment $comments $Owner
    if ($null -eq $correction) { return $null }

    $lastVerdict = Find-LatestVerdictComment $comments
    if ($null -ne $lastVerdict -and
        [DateTimeOffset]$correction.createdAt -le [DateTimeOffset]$lastVerdict.createdAt) {
        return $null
    }

    $state = [string]$Issue.state
    $recheck = $false
    if ($state -eq 'CLOSED') {
        # A closed issue is only re-checked when it already had a verdict and a
        # new correction arrives after the close. Issues closed manually before
        # their first correction stay closed.
        if ($null -eq $lastVerdict) { return $null }
        if (-not $Issue.closedAt) { return $null }
        if ([DateTimeOffset]$correction.createdAt -le [DateTimeOffset]$Issue.closedAt) {
            return $null
        }
        $recheck = $true
    } elseif ($state -ne 'OPEN') {
        return $null
    }

    return [pscustomobject]@{
        correction = $correction
        lastVerdict = $lastVerdict
        recheck = $recheck
    }
}
