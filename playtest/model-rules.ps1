# Shared model allowlist and OpenRouter provider-routing helpers for playtest,
# verify, and triage scripts.

$script:PlaytestModelsAllowlistPath = Join-Path $PSScriptRoot "models-allowlist.txt"
$script:MandatoryRelaceModel = "openrouter/z-ai/glm-5.3-flash"
$script:MandatoryRelaceOpenRouterModel = "z-ai/glm-5.3-flash"
$script:MandatoryRelaceProviderOnly = @("relace/fp4")

function Get-PlaytestAllowlistedModels([string]$AllowlistPath = $script:PlaytestModelsAllowlistPath) {
    if (-not (Test-Path $AllowlistPath)) {
        return @()
    }
    $items = [System.Collections.Generic.List[string]]::new()
    foreach ($line in Get-Content $AllowlistPath) {
        $trimmed = $line.Trim()
        if (-not $trimmed -or $trimmed.StartsWith("#")) { continue }
        $items.Add($trimmed)
    }
    return $items.ToArray()
}

function Assert-AllowedPlaytestModels(
    [string[]]$Models,
    [string]$AllowlistPath = $script:PlaytestModelsAllowlistPath
) {
    $normalized = @($Models | ForEach-Object { $_ -split "," } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    if (-not $normalized.Count) {
        throw "At least one allowed model is required"
    }
    $allowlisted = @(Get-PlaytestAllowlistedModels $AllowlistPath)
    $unexpectedAllowlist = @($allowlisted | Where-Object { $_ -ne $script:MandatoryRelaceModel })
    if ($unexpectedAllowlist.Count) {
        throw "models-allowlist.txt may only allow '$($script:MandatoryRelaceModel)'; found: $($unexpectedAllowlist -join ', ')"
    }
    $invalid = @(
        $normalized | Where-Object {
            ($_ -notmatch "(-free|:free)$") -and ($allowlisted -notcontains $_)
        }
    )
    if ($invalid.Count) {
        throw "Only free models (-free/:free) or allowlisted '$($script:MandatoryRelaceModel)' are allowed; invalid model id(s): $($invalid -join ', ')"
    }
    return $normalized
}

function Add-OpenCodeProviderRoutingConfig(
    [System.Collections.IDictionary]$Config,
    [string[]]$Models,
    [string]$ActiveModel = ""
) {
    $usesMandatoryModel = ($Models -contains $script:MandatoryRelaceModel) -or
        ($ActiveModel -eq $script:MandatoryRelaceModel)
    if ($usesMandatoryModel) {
        # OpenCode provider model options pass through to OpenRouter's request body
        # (documented at https://opencode.ai/docs/providers/ and verified against
        # opencode v2.2.7).
        if (-not $Config.Contains("provider") -or $null -eq $Config["provider"]) {
            $Config["provider"] = [ordered]@{}
        }
        if (-not $Config["provider"].Contains("openrouter") -or $null -eq $Config["provider"]["openrouter"]) {
            $Config["provider"]["openrouter"] = [ordered]@{}
        }
        if (-not $Config["provider"]["openrouter"].Contains("models") -or $null -eq $Config["provider"]["openrouter"]["models"]) {
            $Config["provider"]["openrouter"]["models"] = [ordered]@{}
        }
        $Config["provider"]["openrouter"]["models"][$script:MandatoryRelaceOpenRouterModel] = [ordered]@{
            options = [ordered]@{
                provider = [ordered]@{
                    only = @($script:MandatoryRelaceProviderOnly)
                    allow_fallbacks = $false
                }
            }
        }
    }
    if ($ActiveModel -eq $script:MandatoryRelaceModel) {
        # Prevent OpenCode's background title/small_model agent from falling back
        # to a default model outside relace/fp4.
        $Config["small_model"] = $script:MandatoryRelaceModel
    } elseif (-not [string]::IsNullOrWhiteSpace($ActiveModel)) {
        $Config["small_model"] = $ActiveModel
    }
    return $Config
}
