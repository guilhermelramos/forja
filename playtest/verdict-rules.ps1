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
$script:BotCloseActor = 'github-actions[bot]'

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

# An issue has something to verify when its newest owner correction is newer
# than its newest verdict. This ignores the issue state and the close actor.
function Get-PendingCorrection([object]$Issue, [string]$Owner) {
    if ($null -eq $Issue) { return $null }
    $comments = @(Get-SortedComments $Issue)
    $correction = Find-LatestCorrectionComment $comments $Owner
    if ($null -eq $correction) { return $null }

    $lastVerdict = Find-LatestVerdictComment $comments
    if ($null -ne $lastVerdict -and
        [DateTimeOffset]$correction.createdAt -le [DateTimeOffset]$lastVerdict.createdAt) {
        return $null
    }

    return [pscustomobject]@{
        correction = $correction
        lastVerdict = $lastVerdict
    }
}

# Returns the pending correction for an open issue, or for a closed issue that
# already had a verdict, was closed by github-actions[bot], and got a newer
# correction after the close. The caller must resolve `closeActor` first with
# Get-IssueCloseActor. An issue closed by a person is never a re-check.
function Get-PendingVerification([object]$Issue, [string]$Owner) {
    $pending = Get-PendingCorrection $Issue $Owner
    if ($null -eq $pending) { return $null }

    $state = [string]$Issue.state
    $recheck = $false
    if ($state -eq 'CLOSED') {
        # A closed issue is only re-checked when the verifier closed it and a
        # new correction arrives after the close. Issues closed manually (even
        # before their first correction) stay closed.
        if ($null -eq $pending.lastVerdict) { return $null }
        if (-not $Issue.closedAt) { return $null }
        if ([DateTimeOffset]$pending.correction.createdAt -le [DateTimeOffset]$Issue.closedAt) {
            return $null
        }
        if ([string]$Issue.closeActor -ne $script:BotCloseActor) { return $null }
        $recheck = $true
    } elseif ($state -ne 'OPEN') {
        return $null
    }

    return [pscustomobject]@{
        correction = $pending.correction
        lastVerdict = $pending.lastVerdict
        recheck = $recheck
    }
}

function Get-LastCloseActor([object[]]$Events) {
    $closes = @($Events | Where-Object { $_ -and $_.event -eq 'closed' } |
        Sort-Object { [DateTimeOffset]$_.created_at })
    if (-not $closes.Count) { return $null }
    return [string]$closes[-1].actor.login
}

# Reads the issue event timeline and returns the actor of the last `closed`
# event. `--slurp` is required because GitHub paginates the event list.
function Get-IssueCloseActor([string]$Repository, [int]$Number) {
    $slurped = & gh api "repos/$Repository/issues/$Number/events?per_page=100" --paginate --slurp
    if ($LASTEXITCODE -ne 0) { throw "could not read events for issue #$Number" }
    $events = @()
    foreach ($page in @($slurped | ConvertFrom-Json)) { $events += @($page) }
    return Get-LastCloseActor $events
}
