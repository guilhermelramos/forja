# Offline fake OpenCode script used only by playtest/test-triage.ps1.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)

$ErrorActionPreference = "Stop"
$model = ""
for ($i = 0; $i -lt $Arguments.Count; $i++) {
    if ($Arguments[$i] -eq "--model" -and $i + 1 -lt $Arguments.Count) {
        $model = $Arguments[$i + 1]
        $i++
    }
}
$prompt = $Arguments[-1]

$issueNumber = 0
if ($prompt -match 'Número:\s*#(\d+)') {
    $issueNumber = [int]$Matches[1]
}

switch ($issueNumber) {
    710 {
        # Model deliberately suggests priority 3, shortcut must force 5.
        '{"priority":3,"area":"area-explorador","rationale":"Fechamento inesperado ao clicar na lista de extensões.","duplicate_of":null}'
    }
    740 {
        # Model suggests 2, shortcut for "travou" must force 5.
        '{"priority":2,"area":"area-terminal","rationale":"Interface travou por mais de 10 segundos na tela com Terminal.","duplicate_of":null}'
    }
    690 {
        # Model suggests 5, shortcut for `ideia` without data loss must cap at 3.
        '{"priority":5,"area":"area-uso","rationale":"Melhoria para exibir custos e quebras na tela de Uso.","duplicate_of":null}'
    }
    691 {
        # Ideia WITH data loss may stay at 5.
        '{"priority":5,"area":"area-editor","rationale":"Risco de perda de dados ao fechar aba modificada do editor.","duplicate_of":null}'
    }
    801 {
        # Model suggests 4, shortcut for explore nameless accessible must force 2.
        '{"priority":4,"area":"area-bigorna","rationale":"Elemento clicável na Bigorna sem nome acessível para leitor de tela.","duplicate_of":null}'
    }
    901 {
        '{"priority":4,"area":"area-editor","rationale":"Atalho de salvar falha ao alternar abas do editor.","duplicate_of":null}'
    }
    902 {
        '{"priority":4,"area":"area-editor","rationale":"Retriagem reclassifica para editor mantendo acompanhamento.","duplicate_of":901}'
    }
    905 {
        # Invalid JSON output.
        'Desculpe, não consegui gerar JSON válido {priority: broken'
    }
    906 {
        # Out-of-range priority (6).
        '{"priority":6,"area":"area-ajustes","rationale":"Prioridade inválida fora de 1 a 5.","duplicate_of":null}'
    }
    907 {
        if ($model -eq "mock/rate-limited-free") {
            [Console]::Error.WriteLine("Error: Free usage limit reached.")
            exit 1
        }
        '{"priority":4,"area":"area-chat","rationale":"Modelo reserva classificou o problema no chat.","duplicate_of":null}'
    }
    default {
        '{"priority":3,"area":"area-outros","rationale":"Classificação padrão de fixture.","duplicate_of":null}'
    }
}
