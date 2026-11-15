# Fixture tests for automatic issue triage rules and playtest/triage.ps1.
# Covers: non-AI shortcuts, invalid JSON, out-of-range priority, preserving
# `em-andamento`, skipping closed/already-triaged issues, and -DryRun without writes.
# Run: pwsh -NoProfile -File ./playtest/test-triage.ps1

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "triage-rules.ps1")
. (Join-Path $PSScriptRoot "model-rules.ps1")

$fixturesFile = Join-Path $PSScriptRoot "fixtures/triage/issues.json"
$issues = @(Get-Content $fixturesFile -Raw | ConvertFrom-Json)
$byNumber = @{}
foreach ($item in $issues) { $byNumber[[int]$item.number] = $item }

# 1. Unit tests for non-AI shortcuts (before calling AI).
$scCrash = Get-TriageShortcut $byNumber[710]
if ($scCrash.MinPriority -ne 5 -or $scCrash.FixedPriority -ne 5) {
    throw "shortcut #710 (kind crash): expected min/fixed 5, got min=$($scCrash.MinPriority) fixed=$($scCrash.FixedPriority)"
}

$scTravou = Get-TriageShortcut $byNumber[740]
if ($scTravou.MinPriority -ne 5 -or $scTravou.FixedPriority -ne 5) {
    throw "shortcut #740 (travou): expected min/fixed 5, got min=$($scTravou.MinPriority) fixed=$($scTravou.FixedPriority)"
}

$scIdeia = Get-TriageShortcut $byNumber[690]
if ($scIdeia.MaxPriority -ne 3) {
    throw "shortcut #690 (ideia without data loss): expected max 3, got $($scIdeia.MaxPriority)"
}

$scIdeiaLoss = Get-TriageShortcut $byNumber[691]
if ($scIdeiaLoss.MaxPriority -ne 5) {
    throw "shortcut #691 (ideia with data loss): expected max 5, got $($scIdeiaLoss.MaxPriority)"
}

$scNameless = Get-TriageShortcut $byNumber[801]
if ($scNameless.MinPriority -ne 2 -or $scNameless.MaxPriority -ne 2 -or $scNameless.FixedPriority -ne 2) {
    throw "shortcut #801 (explore nameless): expected fixed 2, got min=$($scNameless.MinPriority) max=$($scNameless.MaxPriority)"
}

$altNameless = [pscustomobject]@{
    number = 802
    title = "[explore] elemento sem nome acessível em 'Ajustes'"
    body = ""
    state = "OPEN"
    labels = @("explore")
}
if ((Get-TriageShortcut $altNameless).FixedPriority -ne 2) {
    throw "shortcut #802 (exact phrase 'elemento sem nome acessível'): expected fixed 2"
}

# 2. Unit tests for JSON validation and out-of-range priorities.
$badJson = ConvertFrom-TriageModelOutput "texto livre sem json" 905
if ($badJson.Valid) { throw "invalid JSON must be rejected" }

$truncatedJson = ConvertFrom-TriageModelOutput '{"priority":3,"area":' 905
if ($truncatedJson.Valid) { throw "truncated JSON must be rejected" }

foreach ($badPriority in @(0, 6, -1, 3.5, '"3"', 'true')) {
    $raw = "{`"priority`":$badPriority,`"area`":`"area-ajustes`",`"rationale`":`"Teste.`",`"duplicate_of`":null}"
    $res = ConvertFrom-TriageModelOutput $raw 906
    if ($res.Valid) { throw "priority '$badPriority' must be rejected" }
}

$badArea = ConvertFrom-TriageModelOutput '{"priority":3,"area":"area-fantasma","rationale":"Teste.","duplicate_of":null}' 906
if ($badArea.Valid) { throw "unknown area must be rejected" }

$fencedValid = ConvertFrom-TriageModelOutput "``````json`n{`"priority`":4,`"area`":`"area-editor`",`"rationale`":`"Falha no salvamento.`",`"duplicate_of`":null}`n``````" 901
if (-not $fencedValid.Valid -or $fencedValid.Priority -ne 4 -or $fencedValid.Area -ne "area-editor") {
    throw "fenced valid JSON should parse cleanly"
}

# 3. Eligibility checks: closed and already-triaged issues.
if (Test-TriageEligibleIssue $byNumber[903] -Retriage) {
    throw "closed issue #903 must never be eligible even with -Retriage"
}
if (Test-TriageEligibleIssue $byNumber[904]) {
    throw "already-triaged #904 must be skipped when -Retriage is false"
}
if (-not (Test-TriageEligibleIssue $byNumber[904] -Retriage)) {
    throw "already-triaged #904 should be eligible when -Retriage is true"
}
if (-not (Test-TriageEligibleIssue $byNumber[901])) {
    throw "open issue #901 with em-andamento and no priority must be eligible"
}

# 4. End-to-end tests with fake gh + fake opencode.
$fakeDir = Join-Path ([System.IO.Path]::GetTempPath()) "forja-triage-test-$PID"
$oldPath = $env:PATH
$oldFixtures = $env:FORJA_TRIAGE_FIXTURES
$oldWrites = $env:FORJA_FAKE_GH_WRITES
$oldOpencode = $env:FORJA_OPENCODE_SCRIPT
$oldMutate = $env:FORJA_FAKE_GH_MUTATE
$oldOrKey = $env:OPENROUTER_API_KEY
$oldOrBase = $env:OPENROUTER_BASE_URL

try {
    $null = New-Item -ItemType Directory -Force $fakeDir
    Copy-Item (Join-Path $PSScriptRoot "fixtures/triage/fake-gh.ps1") $fakeDir
    Set-Content -Path (Join-Path $fakeDir "gh.cmd") -Encoding ascii -Value @"
@echo off
pwsh -NoProfile -File "%~dp0fake-gh.ps1" %*
"@
    $env:PATH = "$fakeDir;$oldPath"
    $env:FORJA_TRIAGE_FIXTURES = $fixturesFile
    $env:FORJA_OPENCODE_SCRIPT = (Join-Path $PSScriptRoot "fixtures/triage/fake-opencode.ps1")

    # 4a. -DryRun must succeed with zero writes (FORJA_FAKE_GH_WRITES unset).
    Remove-Item Env:FORJA_FAKE_GH_WRITES -ErrorAction SilentlyContinue
    $dryRunOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -MaxIssues 30 -DryRun `
        -Models "mock/rate-limited-free,mock/primary-free" *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "DryRun failed with exit code $LASTEXITCODE`n$dryRunOut" }
    if ($dryRunOut -notmatch "DRY RUN #710: add \[prioridade-5, area-explorador\]") {
        throw "DryRun #710 did not enforce crash shortcut priority 5:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #740: add \[prioridade-5, area-terminal\]") {
        throw "DryRun #740 did not enforce travou shortcut priority 5:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #690: add \[prioridade-3, area-uso\]") {
        throw "DryRun #690 did not enforce ideia max 3:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #691: add \[prioridade-5, area-editor\]") {
        throw "DryRun #691 did not allow priority 5 for ideia with data loss:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #801: add \[prioridade-2, area-bigorna\]") {
        throw "DryRun #801 did not enforce explore nameless shortcut priority 2:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #901:.*preserving em-andamento.*Triagem: prioridade 4 \(área area-editor\)") {
        throw "DryRun #901 did not preserve em-andamento or format one-line comment:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "DRY RUN #907: add \[prioridade-4, area-chat\]") {
        throw "DryRun #907 did not fall back to second model:`n$dryRunOut"
    }
    if ($dryRunOut -match "DRY RUN #903" -or $dryRunOut -match "DRY RUN #904" -or
        $dryRunOut -match "DRY RUN #905" -or $dryRunOut -match "DRY RUN #906") {
        throw "DryRun included closed/already-triaged/invalid issues:`n$dryRunOut"
    }
    if ($dryRunOut -notmatch "ignoring invalid model output") {
        throw "DryRun did not log ignored invalid/out-of-range outputs:`n$dryRunOut"
    }

    # 4b. Live-write mode against fake-gh: verify exact writes, preserved em-andamento,
    # invalid JSON/out-of-range produce zero writes, and closed #903 is never touched.
    $writeLog = Join-Path $fakeDir "writes.txt"
    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $env:FORJA_FAKE_GH_WRITES = $writeLog

    $applyOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -MaxIssues 30 `
        -Models @("mock/primary-free") *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "Apply run failed:`n$applyOut" }
    $writes = @(Get-Content $writeLog)
    if ($writes -match "903" -or $writes -match "904" -or $writes -match "905" -or $writes -match "906") {
        throw "Unexpected writes for skipped/invalid issues:`n$($writes -join "`n")"
    }
    if ($writes -match "--remove-label em-andamento" -or $writes -match "label create em-andamento") {
        throw "Triager touched protected label em-andamento:`n$($writes -join "`n")"
    }
    $comment901 = $writes | Where-Object { $_ -match "^issue comment 901 " } | Select-Object -First 1
    if ($comment901 -notmatch "Triagem: prioridade 4 \(área area-editor\) — ") {
        throw "Missing expected single-line comment on #901:`n$($writes -join "`n")"
    }

    # 4c. Closed #903 explicitly targeted with -IssueNumber -Retriage must produce 0 writes.
    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $closedOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -IssueNumber 903 -Retriage -Models @("mock/primary-free") *>&1 | Out-String
    if ($closedOut -notmatch "skip #903: issue is closed") {
        throw "Closed issue #903 was not skipped cleanly:`n$closedOut"
    }
    if (Test-Path $writeLog) {
        throw "Closed issue #903 triggered writes: $(Get-Content $writeLog -Raw)"
    }

    # 4d. -Retriage on #902 replaces prioridade-2/area-layout, adds duplicata?, preserves em-andamento.
    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $retriageOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -IssueNumber 902 -Retriage -Models @("mock/primary-free") *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "Retriage #902 failed:`n$retriageOut" }
    $retriageWrites = @(Get-Content $writeLog)
    $edit902 = $retriageWrites | Where-Object { $_ -match "^issue edit 902 " } | Select-Object -First 1
    if (-not $edit902) { throw "Retriage #902 did not edit labels:`n$($retriageWrites -join "`n")" }
    if ($edit902 -notmatch "--add-label prioridade-4" -or $edit902 -notmatch "--add-label area-editor" -or $edit902 -notmatch "--add-label duplicata\?") {
        throw "Retriage #902 missing expected added labels: $edit902"
    }
    if ($edit902 -notmatch "--remove-label prioridade-2" -or $edit902 -notmatch "--remove-label area-layout") {
        throw "Retriage #902 did not remove stale priority/area labels: $edit902"
    }
    if ($edit902 -match "em-andamento") {
        throw "Retriage #902 must NEVER touch em-andamento: $edit902"
    }

    # 5. Unit tests for deterministic provisional priority and area rules.
    $provisionalCases = @(
        @{
            Name = "crash via kind"
            Issue = $byNumber[710]
            Priority = 5
            Area = "area-explorador"
        },
        @{
            Name = "travou in title"
            Issue = $byNumber[740]
            Priority = 5
            Area = "area-terminal"
        },
        @{
            Name = "Escape nao fechou"
            Issue = [pscustomobject]@{
                number = 1002; title = "[explore] Escape não fechou janela aberta em 'Ajustes'"
                body = "**Tela:** Ajustes"; state = "OPEN"; labels = @("playtest", "explore")
            }
            Priority = 4
            Area = "area-teclado"
        },
        @{
            Name = "nao abre em fluxo principal"
            Issue = [pscustomobject]@{
                number = 1003; title = "[playtest] Botão Nova bancada não abre diálogo"
                body = "**Tela:** Bancada"; state = "OPEN"; labels = @("playtest")
            }
            Priority = 4
            Area = "area-bancadas"
        },
        @{
            Name = "nao faz nada em fluxo principal"
            Issue = [pscustomobject]@{
                number = 1004; title = "[playtest] Salvar no Editor não faz nada"
                body = "**Tela:** Editor"; state = "OPEN"; labels = @("playtest")
            }
            Priority = 4
            Area = "area-editor"
        },
        @{
            Name = "nao abre fora de fluxo principal -> demais 3"
            Issue = [pscustomobject]@{
                number = 1005; title = "[playtest] Painel Portas não abre"
                body = "**Tela:** Portas"; state = "OPEN"; labels = @("playtest")
            }
            Priority = 3
            Area = "area-explorador"
        },
        @{
            Name = "Erro tecnico cru"
            Issue = [pscustomobject]@{
                number = 1006; title = "[explore] Erro técnico cru na tela (os error 2) em 'Chat'"
                body = "**Tela:** Chat"; state = "OPEN"; labels = @("playtest", "explore")
            }
            Priority = 3
            Area = "area-textos"
        },
        @{
            Name = "Elemento fora da janela"
            Issue = [pscustomobject]@{
                number = 1007; title = "[explore] Elemento fora da janela: button `"Ok`" em 'Terminal'"
                body = "**Tela:** Terminal"; state = "OPEN"; labels = @("playtest", "explore")
            }
            Priority = 2
            Area = "area-layout"
        },
        @{
            Name = "sobrepostos"
            Issue = [pscustomobject]@{
                number = 1008; title = "[explore] Abas/botões sobrepostos na mesma faixa"
                body = "**Tela:** Geral"; state = "OPEN"; labels = @("playtest", "explore")
            }
            Priority = 2
            Area = "area-layout"
        },
        @{
            Name = "sem nome acessivel"
            Issue = $byNumber[801]
            Priority = 2
            Area = "area-bigorna"
        },
        @{
            Name = "ideia padrao"
            Issue = $byNumber[690]
            Priority = 2
            Area = "area-uso"
        },
        @{
            Name = "ideia citando perda de tempo"
            Issue = [pscustomobject]@{
                number = 1011; title = "[ideia] Melhoria: atalho no Git evita perda de tempo"
                body = "**Tela:** Mudanças`n**Aconteceu:** Muita perda de tempo abrindo diff."; state = "OPEN"; labels = @("ideia")
            }
            Priority = 3
            Area = "area-git"
        },
        @{
            Name = "ideia citando fluxo principal"
            Issue = [pscustomobject]@{
                number = 1012; title = "[ideia] Melhoria no fluxo principal de criação de bancada"
                body = "**Tela:** Bancadas"; state = "OPEN"; labels = @("ideia")
            }
            Priority = 3
            Area = "area-bancadas"
        },
        @{
            Name = "demais sem area conhecida"
            Issue = [pscustomobject]@{
                number = 1010; title = "[playtest] Pequeno defeito inesperado na transição"
                body = "**Tela:** Geral"; state = "OPEN"; labels = @("playtest")
            }
            Priority = 3
            Area = "area-outros"
        },
        @{
            Name = "area ajudantes"
            Issue = [pscustomobject]@{
                number = 1013; title = "[playtest] Perfil de ajudante não detectado"
                body = "**Tela:** Ajudantes"; state = "OPEN"; labels = @("playtest")
            }
            Priority = 3
            Area = "area-ajudantes"
        },
        @{
            Name = "area mutirao"
            Issue = [pscustomobject]@{
                number = 1014; title = "[playtest] Mutirão não mostra recados"
                body = "**Tela:** Mutirão"; state = "OPEN"; labels = @("playtest")
            }
            Priority = 3
            Area = "area-mutirao"
        },
        @{
            Name = "area navegador"
            Issue = [pscustomobject]@{
                number = 1015; title = "[playtest] Navegador não abre página local"
                body = "**Tela:** Navegador"; state = "OPEN"; labels = @("playtest")
            }
            Priority = 4
            Area = "area-navegador"
        },
        @{
            Name = "area rotinas"
            Issue = [pscustomobject]@{
                number = 1016; title = "[playtest] Histórico de rotinas vazio"
                body = "**Tela:** Rotinas"; state = "OPEN"; labels = @("playtest")
            }
            Priority = 3
            Area = "area-rotinas"
        },
        @{
            Name = "area ajustes"
            Issue = [pscustomobject]@{
                number = 1017; title = "[playtest] Preferência não persiste após reiniciar"
                body = "**Tela:** Ajustes"; state = "OPEN"; labels = @("playtest")
            }
            Priority = 3
            Area = "area-ajustes"
        }
    )
    foreach ($case in $provisionalCases) {
        $p = Resolve-ProvisionalTriagePlan $case.Issue
        if ($p.Priority -ne $case.Priority -or $p.Area -ne $case.Area) {
            throw "Provisional rule '$($case.Name)' failed: expected P$($case.Priority) $($case.Area), got P$($p.Priority) $($p.Area)"
        }
        if ($p.AddLabels -notcontains "triagem-provisória") {
            throw "Provisional rule '$($case.Name)' missing triagem-provisória label"
        }
        if ($p.Comment -notmatch "^Triagem provisória: prioridade $($case.Priority) \(área $($case.Area)\) — regra .+; será refinada quando houver cota\.$") {
            throw "Provisional rule '$($case.Name)' comment format mismatch: $($p.Comment)"
        }
    }

    # 6. End-to-end stateful fixture test:
    #    Round 1: all models rate-limited -> every eligible issue receives provisional priority + area + triagem-provisória; em-andamento preserved.
    #    Round 1b: all models still rate-limited -> no duplicate comments on already-provisional issues.
    #    Round 2: a model responds (without -Retriage) -> refines ONLY triagem-provisória issues, removes triagem-provisória, skips #904 and closed #903, preserves em-andamento.
    $statefulFixtures = Join-Path $fakeDir "stateful-issues.json"
    $statefulSet = @(
        $byNumber[710],
        $byNumber[903],
        $byNumber[904],
        [pscustomobject]@{
            number = 1003; title = "[playtest] Botão Nova bancada não abre diálogo"
            body = "**Tela:** Bancada"; state = "OPEN"; url = "https://github.com/fixture/repo/issues/1003"
            labels = @([pscustomobject]@{ name = "playtest" })
        },
        [pscustomobject]@{
            number = 1009; title = "[playtest] Problema genérico em andamento sem área clara"
            body = "**Tela:** Geral"; state = "OPEN"; url = "https://github.com/fixture/repo/issues/1009"
            labels = @([pscustomobject]@{ name = "playtest" }, [pscustomobject]@{ name = "em-andamento" })
        },
        [pscustomobject]@{
            number = 1010; title = "[ideia] Ideia sem perda de tempo e sem área específica"
            body = "**Tela:** Geral"; state = "OPEN"; url = "https://github.com/fixture/repo/issues/1010"
            labels = @([pscustomobject]@{ name = "ideia" })
        }
    )
    ConvertTo-Json -InputObject @($statefulSet) -Depth 20 | Set-Content -Path $statefulFixtures -Encoding utf8
    $env:FORJA_TRIAGE_FIXTURES = $statefulFixtures
    $env:FORJA_FAKE_GH_MUTATE = "1"
    Remove-Item $writeLog -ErrorAction SilentlyContinue

    $provRunOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -MaxIssues 30 `
        -Models "mock/always-rate-limited-free,mock/always-rate-limited-2-free" *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "All-rate-limited round failed:`n$provRunOut" }

    $provWrites = @(Get-Content $writeLog)
    if ($provWrites -match "903" -or $provWrites -match "904") {
        throw "Provisional round touched closed #903 or already-triaged #904:`n$($provWrites -join "`n")"
    }
    if ($provWrites -match "--remove-label em-andamento" -or $provWrites -match "label create em-andamento") {
        throw "Provisional round touched protected em-andamento:`n$($provWrites -join "`n")"
    }
    foreach ($expectedNum in @(710, 1003, 1009, 1010)) {
        $editLine = $provWrites | Where-Object { $_ -match "^issue edit $expectedNum " } | Select-Object -First 1
        $commentLine = $provWrites | Where-Object { $_ -match "^issue comment $expectedNum " } | Select-Object -First 1
        if (-not $editLine -or $editLine -notmatch "--add-label triagem-provisória" -or $editLine -notmatch "--add-label prioridade-[1-5]") {
            throw "Issue #$expectedNum did not receive provisional labels:`n$($provWrites -join "`n")"
        }
        if (-not $commentLine -or $commentLine -notmatch "Triagem provisória: prioridade [1-5] \(área area-[a-z]+\) — regra .+; será refinada quando houver cota\.") {
            throw "Issue #$expectedNum missing provisional comment:`n$($provWrites -join "`n")"
        }
    }

    $afterProv = @(Get-Content $statefulFixtures -Raw | ConvertFrom-Json)
    $byNumAfterProv = @{}
    foreach ($item in $afterProv) { $byNumAfterProv[[int]$item.number] = $item }
    foreach ($expectedNum in @(710, 1003, 1009, 1010)) {
        $lbls = @(Get-IssueLabelNames $byNumAfterProv[$expectedNum])
        if (-not ($lbls -contains "triagem-provisória") -or @($lbls | Where-Object { $_ -match "^prioridade-[1-5]$" }).Count -ne 1) {
            throw "Issue #$expectedNum missing expected persisted labels after provisional round: $($lbls -join ', ')"
        }
    }
    if (@(Get-IssueLabelNames $byNumAfterProv[1009]) -notcontains "em-andamento") {
        throw "Issue #1009 lost em-andamento after provisional round"
    }

    # Round 1b: running again while all models are still rate-limited must not spam duplicate comments.
    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $repeatProvOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -MaxIssues 30 `
        -Models @("mock/always-rate-limited-free") *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "Repeat all-rate-limited round failed:`n$repeatProvOut" }
    if (Test-Path $writeLog) {
        throw "Repeat all-rate-limited round wrote unexpected changes: $(Get-Content $writeLog -Raw)"
    }

    # Round 2: model responds WITHOUT -Retriage -> refines ONLY triagem-provisória issues, removes triagem-provisória, preserves em-andamento.
    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $refineOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -MaxIssues 30 `
        -Models @("mock/primary-free") *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "Refinement round failed:`n$refineOut" }
    $refineWrites = @(Get-Content $writeLog)
    if ($refineWrites -match "903" -or $refineWrites -match "904") {
        throw "Refinement round touched closed #903 or non-provisional #904:`n$($refineWrites -join "`n")"
    }
    if ($refineWrites -match "--remove-label em-andamento" -or $refineWrites -match "label create em-andamento") {
        throw "Refinement round touched protected em-andamento:`n$($refineWrites -join "`n")"
    }

    # #1009 was provisional (prioridade-3, area-outros) and model refines to (prioridade-4, area-editor), keeping em-andamento.
    $edit1009 = $refineWrites | Where-Object { $_ -match "^issue edit 1009 " } | Select-Object -First 1
    if ($edit1009 -notmatch "--add-label prioridade-4" -or $edit1009 -notmatch "--add-label area-editor" -or
        $edit1009 -notmatch "--remove-label triagem-provisória" -or
        $edit1009 -notmatch "--remove-label prioridade-3" -or $edit1009 -notmatch "--remove-label area-outros") {
        throw "Issue #1009 refinement labels mismatch: $edit1009"
    }
    # #1010 agreed with provisional (prioridade-2, area-outros), must STILL remove triagem-provisória.
    $edit1010 = $refineWrites | Where-Object { $_ -match "^issue edit 1010 " } | Select-Object -First 1
    if ($edit1010 -notmatch "--remove-label triagem-provisória") {
        throw "Issue #1010 did not remove triagem-provisória when model agreed with provisional labels: $edit1010"
    }

    $afterRefine = @(Get-Content $statefulFixtures -Raw | ConvertFrom-Json)
    $byNumAfterRefine = @{}
    foreach ($item in $afterRefine) { $byNumAfterRefine[[int]$item.number] = $item }
    foreach ($expectedNum in @(710, 1003, 1009, 1010)) {
        $lbls = @(Get-IssueLabelNames $byNumAfterRefine[$expectedNum])
        if ($lbls -contains "triagem-provisória") {
            throw "Issue #$expectedNum still has triagem-provisória after refinement: $($lbls -join ', ')"
        }
    }
    if (@(Get-IssueLabelNames $byNumAfterRefine[1009]) -notcontains "em-andamento") {
        throw "Issue #1009 lost em-andamento after refinement"
    }

    # Round 3: another run without -Retriage now finds 0 eligible issues.
    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $doneOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -MaxIssues 30 `
        -Models @("mock/primary-free") *>&1 | Out-String
    if ($LASTEXITCODE -ne 0 -or $doneOut -notmatch "no eligible open issues found") {
        throw "Expected 0 eligible issues after refinement:`n$doneOut"
    }

    # 7. Timeout fallback test: all models time out -> provisional triage; timeout + working model -> normal triage.
    $timeoutFixtures = Join-Path $fakeDir "timeout-issues.json"
    ConvertTo-Json -InputObject @($byNumber[740]) -Depth 20 | Set-Content -Path $timeoutFixtures -Encoding utf8
    $env:FORJA_TRIAGE_FIXTURES = $timeoutFixtures
    $env:FORJA_FAKE_GH_MUTATE = "0"
    Remove-Item $writeLog -ErrorAction SilentlyContinue

    $timeoutOnlyOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -TimeoutSeconds 1 `
        -Models @("mock/timeout-free") *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "All-timeout run failed:`n$timeoutOnlyOut" }
    $timeoutWrites = @(Get-Content $writeLog)
    $edit740Timeout = $timeoutWrites | Where-Object { $_ -match "^issue edit 740 " } | Select-Object -First 1
    if ($edit740Timeout -notmatch "--add-label prioridade-5" -or $edit740Timeout -notmatch "--add-label area-terminal" -or
        $edit740Timeout -notmatch "--add-label triagem-provisória") {
        throw "All-timeout run did not apply provisional triage to #740: $edit740Timeout"
    }

    Remove-Item $writeLog -ErrorAction SilentlyContinue
    $timeoutFallbackOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -TimeoutSeconds 1 `
        -Models "mock/timeout-free,mock/primary-free" *>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "Timeout+fallback run failed:`n$timeoutFallbackOut" }
    $fallbackWrites = @(Get-Content $writeLog)
    $edit740Fallback = $fallbackWrites | Where-Object { $_ -match "^issue edit 740 " } | Select-Object -First 1
    if ($edit740Fallback -match "triagem-provisória") {
        throw "Timeout+working model run should use working model (not provisional): $edit740Fallback"
    }

    # 8. Model allowlist & OpenCode provider routing config tests.
    $allowed = Assert-AllowedPlaytestModels @("openrouter/z-ai/glm-5.3-flash", "openrouter/google/gemma-4-31b-it:free", "opencode/glm-5.3-flash-free")
    if ($allowed.Count -ne 3 -or $allowed[0] -ne "openrouter/z-ai/glm-5.3-flash") {
        throw "Allowlist failed to accept openrouter/z-ai/glm-5.3-flash + free models"
    }
    $rejectedPaid = $false
    try {
        $null = Assert-AllowedPlaytestModels @("openrouter/openai/gpt-4o")
    } catch {
        $rejectedPaid = $true
    }
    if (-not $rejectedPaid) {
        throw "Assert-AllowedPlaytestModels must reject paid model not in models-allowlist.txt"
    }
    $badAllowlist = Join-Path $fakeDir "bad-allowlist.txt"
    Set-Content -Path $badAllowlist -Value "openrouter/openai/gpt-4o" -Encoding utf8
    $rejectedBadAllowlist = $false
    try {
        $null = Assert-AllowedPlaytestModels @("openrouter/openai/gpt-4o") $badAllowlist
    } catch {
        $rejectedBadAllowlist = $true
    }
    if (-not $rejectedBadAllowlist) {
        throw "models-allowlist.txt must only permit openrouter/z-ai/glm-5.3-flash"
    }

    $cfgWithRelace = Add-OpenCodeProviderRoutingConfig ([ordered]@{}) @("openrouter/z-ai/glm-5.3-flash", "opencode/glm-5.3-flash-free") "openrouter/z-ai/glm-5.3-flash"
    $relaceOpts = $cfgWithRelace.provider.openrouter.models["z-ai/glm-5.3-flash"].options.provider
    if (($relaceOpts.only -join ",") -ne "relace/fp4" -or $relaceOpts.allow_fallbacks -ne $false) {
        throw "OpenCode config missing mandatory relace/fp4 routing: $($cfgWithRelace | ConvertTo-Json -Depth 10 -Compress)"
    }
    if ($cfgWithRelace.small_model -ne "openrouter/z-ai/glm-5.3-flash") {
        throw "OpenCode config must pin small_model to openrouter/z-ai/glm-5.3-flash when active"
    }

    $triagePaidOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
        -Repository "fixture/repo" -DryRun -Models "openrouter/openai/gpt-4o" *>&1 | Out-String
    if ($LASTEXITCODE -eq 0) {
        throw "triage.ps1 must reject non-allowlisted paid models:`n$triagePaidOut"
    }

    # 9. End-to-end TypeScript @openrouter/sdk streaming + relace/fp4 routing + usage summary test.
    $sdkIssues = Join-Path $fakeDir "sdk-issues.json"
    ConvertTo-Json -InputObject @($byNumber[901]) -Depth 20 | Set-Content -Path $sdkIssues -Encoding utf8
    $env:FORJA_TRIAGE_FIXTURES = $sdkIssues
    $env:FORJA_FAKE_GH_MUTATE = "0"
    Remove-Item Env:FORJA_OPENCODE_SCRIPT -ErrorAction SilentlyContinue
    $env:OPENROUTER_API_KEY = "fixture-test-key"

    function Invoke-WithMockOpenRouter([string]$Mode, [scriptblock]$Body) {
        $portFile = Join-Path $fakeDir "or-port-$Mode.txt"
        $captureFile = Join-Path $fakeDir "or-cap-$Mode.jsonl"
        Remove-Item $portFile, $captureFile -ErrorAction SilentlyContinue
        $serverScript = Join-Path $PSScriptRoot "fixtures/triage/mock-openrouter-server.mjs"
        $srvProc = Start-Process -FilePath "node" -ArgumentList @($serverScript, $portFile, $captureFile, $Mode) -PassThru -NoNewWindow
        try {
            $deadline = (Get-Date).AddSeconds(10)
            while (-not (Test-Path $portFile) -and (Get-Date) -lt $deadline) {
                Start-Sleep -Milliseconds 50
            }
            if (-not (Test-Path $portFile)) { throw "Mock OpenRouter server ($Mode) failed to bind" }
            $port = (Get-Content $portFile -Raw).Trim()
            $env:OPENROUTER_BASE_URL = "http://127.0.0.1:$port"
            & $Body $captureFile
        } finally {
            if ($srvProc -and -not $srvProc.HasExited) { $srvProc.Kill($true) }
        }
    }

    Invoke-WithMockOpenRouter "ok" {
        param($captureFile)
        Remove-Item $writeLog -ErrorAction SilentlyContinue
        $sdkOut = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
            -Repository "fixture/repo" -IssueNumber 901 `
            -Models "openrouter/z-ai/glm-5.3-flash" *>&1 | Out-String
        if ($LASTEXITCODE -ne 0) { throw "OpenRouter SDK triage failed:`n$sdkOut" }
        if ($sdkOut -notmatch "usage #901 \(openrouter/z-ai/glm-5\.3-flash, provider=Relace\): input=120, output=45, reasoning=18") {
            throw "Missing per-issue streaming usage + provider log:`n$sdkOut"
        }
        if ($sdkOut -notmatch "token usage summary: input=120, output=45, reasoning=18 \(1 call\(s\)\)") {
            throw "Missing token usage summary:`n$sdkOut"
        }
        $captured = @(Get-Content $captureFile | ForEach-Object { $_ | ConvertFrom-Json })
        if ($captured.Count -ne 1) { throw "Expected 1 HTTP request to OpenRouter SDK, got $($captured.Count)" }
        $reqBody = $captured[0].body
        if ($reqBody.model -ne "z-ai/glm-5.3-flash" -or $reqBody.stream -ne $true -or
            ($reqBody.provider.only -join ",") -ne "relace/fp4" -or $reqBody.provider.allow_fallbacks -ne $false) {
            throw "OpenRouter SDK request payload mismatch: $($reqBody | ConvertTo-Json -Depth 10 -Compress)"
        }
        $sdkWrites = @(Get-Content $writeLog)
        if (-not ($sdkWrites | Where-Object { $_ -match "^issue edit 901 .*--add-label prioridade-4" })) {
            throw "OpenRouter SDK run did not apply expected labels:`n$($sdkWrites -join "`n")"
        }
    }

    Invoke-WithMockOpenRouter "all429" {
        param($captureFile)
        Remove-Item $writeLog -ErrorAction SilentlyContinue
        $sdk429Out = & pwsh -NoProfile -File (Join-Path $PSScriptRoot "triage.ps1") `
            -Repository "fixture/repo" -IssueNumber 901 `
            -Models "openrouter/z-ai/glm-5.3-flash,openrouter/mock/fallback:free" *>&1 | Out-String
        if ($LASTEXITCODE -ne 0) { throw "OpenRouter SDK all-429 fallback failed:`n$sdk429Out" }
        $writes429 = @(Get-Content $writeLog)
        if (-not ($writes429 | Where-Object { $_ -match "^issue edit 901 .*--add-label triagem-provisória" })) {
            throw "OpenRouter SDK all-429 run did not apply provisional fallback:`n$($writes429 -join "`n")"
        }
    }
} finally {
    $env:PATH = $oldPath
    if ($null -eq $oldFixtures) { Remove-Item Env:FORJA_TRIAGE_FIXTURES -ErrorAction SilentlyContinue } else { $env:FORJA_TRIAGE_FIXTURES = $oldFixtures }
    if ($null -eq $oldWrites) { Remove-Item Env:FORJA_FAKE_GH_WRITES -ErrorAction SilentlyContinue } else { $env:FORJA_FAKE_GH_WRITES = $oldWrites }
    if ($null -eq $oldOpencode) { Remove-Item Env:FORJA_OPENCODE_SCRIPT -ErrorAction SilentlyContinue } else { $env:FORJA_OPENCODE_SCRIPT = $oldOpencode }
    if ($null -eq $oldMutate) { Remove-Item Env:FORJA_FAKE_GH_MUTATE -ErrorAction SilentlyContinue } else { $env:FORJA_FAKE_GH_MUTATE = $oldMutate }
    if ($null -eq $oldOrKey) { Remove-Item Env:OPENROUTER_API_KEY -ErrorAction SilentlyContinue } else { $env:OPENROUTER_API_KEY = $oldOrKey }
    if ($null -eq $oldOrBase) { Remove-Item Env:OPENROUTER_BASE_URL -ErrorAction SilentlyContinue } else { $env:OPENROUTER_BASE_URL = $oldOrBase }
    Remove-Item $fakeDir -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "triage fixtures: all shortcut, provisional, refinement, validation, allowlist, OpenRouter SDK streaming, em-andamento preservation, retriage, fallback, timeout, and DryRun tests passed"
