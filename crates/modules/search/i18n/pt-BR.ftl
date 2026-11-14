search-name = Busca
search-panel-title = Busca
search-command-focus = Focar a busca
search-command-run = Buscar nos arquivos
search-method-run-description = Inicia uma busca de texto nos arquivos da bancada ativa ou pasta informada.
search-method-run-result = Se a busca começou.
search-method-results-description = Devolve o estado e os trechos encontrados da última busca.
search-method-results-result = Estado atual da busca, total de trechos e arquivos.
search-query-hint = Buscar nos arquivos
search-option-regex = Regex
search-option-case = Maiúsculas
search-option-word = Palavra inteira
search-include-hint = Incluir: *.rs, src/**
search-exclude-hint = Excluir: target/**
search-button-search = Buscar
search-button-stop = Parar
search-button-preview = Prévia
search-button-apply = Aplicar
search-button-confirm = Confirmar substituição
search-status-idle = Digite um termo e pressione Enter.
search-status-running = Buscando…
search-status-none = Nenhum resultado.
search-status-results = { $matches ->
    [1] { $matches } resultado
   *[other] { $matches } resultados
} em { $files ->
    [1] { $files } arquivo
   *[other] { $files } arquivos
}
search-status-truncated = Os resultados foram limitados: refine a busca.
search-status-more = Mais resultados não são desenhados: refine a busca.
search-replace-title = Substituir
search-replace-hint = Substituir por
search-preview-summary = { $matches ->
    [1] { $matches } trecho
   *[other] { $matches } trechos
} em { $files ->
    [1] { $files } arquivo
   *[other] { $files } arquivos
}
search-preview-multiline = A mudança atravessa linhas e não é exibida.
search-preview-planning = Calculando a prévia…
search-notice-no_bench = Abra uma bancada primeiro.
search-notice-empty = Digite um termo para buscar.
search-notice-thread = Não foi possível iniciar a busca.
search-notice-no_preview = Faça a prévia da substituição antes de aplicar.
search-notice-replaced = { $matches ->
    [1] { $matches } trecho substituído
   *[other] { $matches } trechos substituídos
} em { $files ->
    [1] { $files } arquivo
   *[other] { $files } arquivos
}.
search-notice-changed = Nada foi gravado: estes arquivos mudaram desde a prévia: { $files }. Faça a prévia de novo.
search-notice-io = Falha ao gravar ({ $message }). Gravados: { $written }; restaurados: { $restored }.
search-notice-error = Não foi possível concluir: { $message }
