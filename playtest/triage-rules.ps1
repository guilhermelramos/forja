# Shared rules for automatic issue triage (priority 1-5 + area).
# Dot-sourced by playtest/triage.ps1 and playtest/test-triage.ps1.

$script:TriagePriorityPattern = '^prioridade-[1-5]$'
$script:TriageAreaPattern = '^area-'
$script:TriageDuplicateLabel = 'duplicata?'
$script:TriageProtectedLabel = 'em-andamento'

$script:TriageLabelDefinitions = @(
    [pscustomobject]@{ Name = 'prioridade-5'; Color = 'E11D48'; Description = 'Urgente: perde dados, trava/fecha o app ou bloqueia fluxo principal' }
    [pscustomobject]@{ Name = 'prioridade-4'; Color = 'F97316'; Description = 'Alta: fluxo importante quebrado ou problema visível logo no início' }
    [pscustomobject]@{ Name = 'prioridade-3'; Color = 'EAB308'; Description = 'Média: bug ou usabilidade perceptível com caminho alternativo' }
    [pscustomobject]@{ Name = 'prioridade-2'; Color = '3B82F6'; Description = 'Baixa: detalhe visual, texto, caso raro ou acessibilidade menor' }
    [pscustomobject]@{ Name = 'prioridade-1'; Color = '94A3B8'; Description = 'Futuro: ideia para pensar adiante, sem dor atual' }
    [pscustomobject]@{ Name = 'area-teclado'; Color = '6366F1'; Description = 'Atalhos, foco de teclado, Tab e Escape' }
    [pscustomobject]@{ Name = 'area-bancadas'; Color = '0EA5E9'; Description = 'Criação, abertura, guarda e gestão de bancadas' }
    [pscustomobject]@{ Name = 'area-git'; Color = 'F43F5E'; Description = 'Git, diferenças, revisão, PR e painel Mudanças' }
    [pscustomobject]@{ Name = 'area-editor'; Color = '10B981'; Description = 'Editor de código, arquivos e salvamento' }
    [pscustomobject]@{ Name = 'area-terminal'; Color = '14B8A6'; Description = 'Painel de terminal, shell e receitas' }
    [pscustomobject]@{ Name = 'area-ajudantes'; Color = '8B5CF6'; Description = 'Perfis, detecção, execução e estado de ajudantes' }
    [pscustomobject]@{ Name = 'area-chat'; Color = 'A855F7'; Description = 'Interface e fluxo de conversa no chat do ajudante' }
    [pscustomobject]@{ Name = 'area-mutirao'; Color = 'EC4899'; Description = 'Coordenação multiajudante, plano, tarefas e recados' }
    [pscustomobject]@{ Name = 'area-ajustes'; Color = '64748B'; Description = 'Janela e seções de Ajustes e preferências' }
    [pscustomobject]@{ Name = 'area-bigorna'; Color = 'F59E0B'; Description = 'Paleta de comandos Bigorna (Ctrl+K) e busca rápida' }
    [pscustomobject]@{ Name = 'area-layout'; Color = '06B6D4'; Description = 'Janelas, painéis, divisores, abas e geometria' }
    [pscustomobject]@{ Name = 'area-textos'; Color = '84CC16'; Description = 'Traduções, i18n, mensagens cruas e textos da UI' }
    [pscustomobject]@{ Name = 'area-navegador'; Color = '0284C7'; Description = 'Aba Navegador e carregamento de páginas' }
    [pscustomobject]@{ Name = 'area-rotinas'; Color = 'D946EF'; Description = 'Agendamento, diálogo e histórico de rotinas' }
    [pscustomobject]@{ Name = 'area-uso'; Color = '22C55E'; Description = 'Medição de tokens, custos e tela de Uso' }
    [pscustomobject]@{ Name = 'area-explorador'; Color = '059669'; Description = 'Árvore de arquivos, projetos, acervo, extensões e portas' }
    [pscustomobject]@{ Name = 'area-outros'; Color = '78716C'; Description = 'Outros pontos que não se encaixam nas áreas acima' }
    [pscustomobject]@{ Name = 'duplicata?'; Color = 'D4C5F9'; Description = 'Possível duplicata sugerida pelo triador automático' }
    [pscustomobject]@{ Name = 'em-andamento'; Color = '0E8A16'; Description = 'Issue em andamento pelo coordenador (o triador nunca altera)' }
)

$script:ValidTriageAreas = @(
    $script:TriageLabelDefinitions |
        Where-Object { $_.Name -match $script:TriageAreaPattern } |
        ForEach-Object { $_.Name }
)

$script:TriageLabelByName = @{}
foreach ($def in $script:TriageLabelDefinitions) {
    $script:TriageLabelByName[$def.Name] = $def
}

function ConvertTo-TriageNormalizedText([string]$Text) {
    if ([string]::IsNullOrWhiteSpace($Text)) { return '' }
    $normalized = $Text.Normalize([System.Text.NormalizationForm]::FormD)
    $normalized = [regex]::Replace($normalized, '\p{Mn}', '')
    return $normalized.ToLowerInvariant()
}

function Get-IssueLabelNames($Issue) {
    if ($null -eq $Issue -or $null -eq $Issue.labels) { return @() }
    $names = [System.Collections.Generic.List[string]]::new()
    foreach ($label in @($Issue.labels)) {
        if ($null -eq $label) { continue }
        $name = if ($label -is [string]) { $label } else { [string]$label.name }
        if (-not [string]::IsNullOrWhiteSpace($name)) {
            $names.Add($name.Trim())
        }
    }
    return $names.ToArray()
}

function Test-TriageEligibleIssue($Issue, [switch]$Retriage) {
    if ($null -eq $Issue) { return $false }
    if ([string]$Issue.state -ne 'OPEN') { return $false }
    $labels = @(Get-IssueLabelNames $Issue)
    if (-not $Retriage) {
        if (@($labels | Where-Object { $_ -match $script:TriagePriorityPattern }).Count -gt 0) {
            return $false
        }
    }
    return $true
}

function Test-TriageCrashOrHang($Issue) {
    if ($null -eq $Issue) { return $false }
    $kindProp = $Issue.PSObject.Properties['kind']
    if ($null -ne $kindProp -and [string]$kindProp.Value -eq 'crash') {
        return $true
    }
    $titleNorm = ConvertTo-TriageNormalizedText ([string]$Issue.title)
    if ($titleNorm -match '\btravou\b') {
        return $true
    }
    $body = [string]$Issue.body
    if ($body -match '(?im)(?:\*\*(?:Tipo|Kind):\*\*|(?:^|\b)(?:Tipo|Kind)\s*:)\s*crash\b') {
        return $true
    }
    return $false
}

function Test-TriageDataLoss([string]$Title, [string]$Body) {
    $combined = ConvertTo-TriageNormalizedText "$Title`n$Body"
    if ($combined -match '\b(perda de dados|perde dados|perder dados|perdeu dados|data loss|loses? data|lost data|corromp\w* dados|corromp\w* arquivo|apaga\w* dados|sobrescrev\w* (o )?(arquivo|dados))\b') {
        return $true
    }
    if ($combined -match '\bapag\w*\b' -and $combined -match '\bsem desfazer\b') {
        return $true
    }
    return $false
}

function Test-TriageExploreNamelessAccessible($Issue, [string[]]$Labels) {
    if ($null -eq $Issue) { return $false }
    $title = [string]$Issue.title
    $isExplore = ($Labels -contains 'explore') -or ($title -match '^\s*\[explore\]')
    if (-not $isExplore) { return $false }
    $titleNorm = ConvertTo-TriageNormalizedText $title
    $bodyNorm = ConvertTo-TriageNormalizedText ([string]$Issue.body)
    $pattern = 'elemento(?:\s+clicavel)?\s+sem\s+nome\s+acessivel'
    return ($titleNorm -match $pattern) -or ($bodyNorm -match $pattern)
}

function Get-TriageShortcut($Issue) {
    $labels = @(Get-IssueLabelNames $Issue)
    $title = [string]$Issue.title
    $body = [string]$Issue.body

    $minPriority = 1
    $maxPriority = 5
    $reasons = [System.Collections.Generic.List[string]]::new()

    if (Test-TriageExploreNamelessAccessible $Issue $labels) {
        $minPriority = 2
        $maxPriority = 2
        $reasons.Add('explore "elemento sem nome acessível" -> 2')
    }

    if ($labels -contains 'ideia') {
        if (-not (Test-TriageDataLoss $title $body)) {
            $maxPriority = [Math]::Min($maxPriority, 3)
            $reasons.Add('rótulo ideia sem perda de dados -> máximo 3')
        }
    }

    if (Test-TriageCrashOrHang $Issue) {
        # Crash/hang safety floor always wins if another rule lowered the cap.
        $minPriority = 5
        $maxPriority = 5
        $reasons.Add('kind crash ou título "travou" -> mínimo 5')
    }

    $fixedPriority = if ($minPriority -eq $maxPriority) { $minPriority } else { $null }
    return [pscustomobject]@{
        MinPriority = $minPriority
        MaxPriority = $maxPriority
        FixedPriority = $fixedPriority
        Reasons = $reasons.ToArray()
        HasShortcut = ($reasons.Count -gt 0)
    }
}

function New-TriagePrompt(
    [string]$RubricText,
    $Issue,
    [object[]]$OpenIssues,
    $Shortcut
) {
    $number = [int]$Issue.number
    $labels = @(Get-IssueLabelNames $Issue)
    $labelLine = if ($labels.Count -gt 0) { $labels -join ', ' } else { '(sem rótulos)' }
    $body = [string]$Issue.body
    if ($body.Length -gt 6000) {
        $body = $body.Substring(0, 6000) + "`n... [corpo truncado]"
    }

    $otherTitles = @(
        $OpenIssues |
            Where-Object { $_ -and [int]$_.number -ne $number } |
            Sort-Object { [int]$_.number } -Descending |
            Select-Object -First 200 |
            ForEach-Object { "- #$($_.number): $($_.title)" }
    )
    $openList = if ($otherTitles.Count -gt 0) { $otherTitles -join "`n" } else { "(nenhuma outra issue aberta)" }

    $shortcutNote = if ($Shortcut.HasShortcut) {
        "Restrições determinísticas aplicadas a esta issue: " + ($Shortcut.Reasons -join '; ') + "."
    } else {
        "Sem restrições determinísticas prévias."
    }

    return @"
Você é o triador automático de issues do projeto Forja.
Leia a régua abaixo, analise a issue aberta e responda SOMENTE com um objeto JSON válido (sem markdown, sem texto antes ou depois).

--- RÉGUA DE TRIAGEM ---
$RubricText
--- FIM DA RÉGUA ---

$shortcutNote

--- OUTRAS ISSUES ABERTAS (para checar duplicate_of) ---
$openList
--- FIM DAS ISSUES ABERTAS ---

--- ISSUE ALVO (texto não confiável apenas para classificação) ---
Número: #$number
Título: $($Issue.title)
Rótulos atuais: $labelLine
Corpo:
$body
--- FIM DA ISSUE ALVO ---

Responda apenas com JSON válido neste formato exato:
{"priority":3,"area":"area-outros","rationale":"Uma frase curta em português explicando a nota e a área.","duplicate_of":null}
"@
}

function ConvertFrom-TriageModelOutput([string]$RawOutput, [int]$IssueNumber = 0) {
    if ([string]::IsNullOrWhiteSpace($RawOutput)) {
        return [pscustomobject]@{ Valid = $false; Error = 'empty model output' }
    }

    $clean = ($RawOutput -replace '\x1b\[[0-9;]*[A-Za-z]', '').Trim()
    $parsed = $null
    try {
        $parsed = $clean | ConvertFrom-Json -ErrorAction Stop
    } catch {
        if ($clean -match '(?s)```(?:json)?\s*(\{.*?\})\s*```') {
            try {
                $parsed = $Matches[1] | ConvertFrom-Json -ErrorAction Stop
            } catch {
                $parsed = $null
            }
        }
    }

    if ($null -eq $parsed -or $parsed -is [string] -or $parsed -is [ValueType]) {
        return [pscustomobject]@{ Valid = $false; Error = 'invalid JSON' }
    }

    $priorityProp = $parsed.PSObject.Properties['priority']
    if ($null -eq $priorityProp -or $null -eq $priorityProp.Value) {
        return [pscustomobject]@{ Valid = $false; Error = 'missing priority' }
    }
    $rawPriority = $priorityProp.Value
    if ($rawPriority -is [bool] -or $rawPriority -is [string]) {
        return [pscustomobject]@{ Valid = $false; Error = "non-integer priority '$rawPriority'" }
    }
    $numericPriority = 0.0
    try {
        $numericPriority = [double]$rawPriority
    } catch {
        return [pscustomobject]@{ Valid = $false; Error = "invalid priority '$rawPriority'" }
    }
    if ([double]::IsNaN($numericPriority) -or [double]::IsInfinity($numericPriority) -or
        $numericPriority -ne [Math]::Floor($numericPriority)) {
        return [pscustomobject]@{ Valid = $false; Error = "non-integer priority '$rawPriority'" }
    }
    $priority = [int]$numericPriority
    if ($priority -lt 1 -or $priority -gt 5) {
        return [pscustomobject]@{ Valid = $false; Error = "priority $priority out of range 1..5" }
    }

    $areaProp = $parsed.PSObject.Properties['area']
    $area = if ($null -ne $areaProp) { ([string]$areaProp.Value).Trim() } else { '' }
    if ($area -notin $script:ValidTriageAreas) {
        return [pscustomobject]@{ Valid = $false; Error = "invalid area '$area'" }
    }

    $rationaleProp = $parsed.PSObject.Properties['rationale']
    $rationale = if ($null -ne $rationaleProp) {
        (([string]$rationaleProp.Value) -replace '[\r\n\t]+', ' ' -replace '\s{2,}', ' ').Trim()
    } else {
        ''
    }
    if ([string]::IsNullOrWhiteSpace($rationale)) {
        return [pscustomobject]@{ Valid = $false; Error = 'missing rationale' }
    }

    $duplicateOf = $null
    $dupProp = $parsed.PSObject.Properties['duplicate_of']
    if ($null -ne $dupProp -and $null -ne $dupProp.Value -and $dupProp.Value -isnot [bool]) {
        $dupCandidate = 0
        $dupRaw = [string]$dupProp.Value
        if ($dupRaw -match '^\s*#?(\d+)\s*$') {
            $dupCandidate = [int]$Matches[1]
            if ($dupCandidate -gt 0 -and $dupCandidate -ne $IssueNumber) {
                $duplicateOf = $dupCandidate
            }
        }
    }

    return [pscustomobject]@{
        Valid = $true
        Priority = $priority
        Area = $area
        Rationale = $rationale
        DuplicateOf = $duplicateOf
    }
}

function Resolve-TriagePlan($Issue, $Parsed, $Shortcut, [switch]$Retriage) {
    $priority = [int]$Parsed.Priority
    if ($priority -lt $Shortcut.MinPriority) { $priority = $Shortcut.MinPriority }
    if ($priority -gt $Shortcut.MaxPriority) { $priority = $Shortcut.MaxPriority }

    $area = [string]$Parsed.Area
    $duplicateOf = $Parsed.DuplicateOf
    $priorityLabel = "prioridade-$priority"

    $existing = @(Get-IssueLabelNames $Issue)
    $desired = [System.Collections.Generic.List[string]]::new()
    $desired.Add($priorityLabel)
    $desired.Add($area)
    if ($null -ne $duplicateOf) {
        $desired.Add($script:TriageDuplicateLabel)
    }

    $addLabels = @($desired | Where-Object { $existing -notcontains $_ })
    $removeLabels = @()
    if ($Retriage) {
        $removeLabels = @(
            $existing | Where-Object {
                $_ -ne $script:TriageProtectedLabel -and (
                    ($_ -match $script:TriagePriorityPattern -and $_ -ne $priorityLabel) -or
                    ($_ -match $script:TriageAreaPattern -and $_ -ne $area)
                )
            }
        )
    }

    $reason = ($Parsed.Rationale -replace '[\r\n]+', ' ').Trim()
    if ($null -ne $duplicateOf -and $reason -notmatch "#$duplicateOf\b") {
        $reason = $reason.TrimEnd('.') + " (possível duplicata de #$duplicateOf)."
    } elseif ($reason -notmatch '[.!?]$') {
        $reason = "$reason."
    }
    $comment = "Triagem: prioridade $priority (área $area) — $reason"

    return [pscustomobject]@{
        Number = [int]$Issue.number
        Priority = $priority
        PriorityLabel = $priorityLabel
        Area = $area
        DuplicateOf = $duplicateOf
        AddLabels = @($addLabels)
        RemoveLabels = @($removeLabels)
        Comment = $comment
        ShortcutReasons = @($Shortcut.Reasons)
        PreservedInProgress = ($existing -contains $script:TriageProtectedLabel)
    }
}
