# counter

Dono das configurações de balcão (`counter.*`, grupo "Pedidos e balcões") e vitrine dos
balcões instalados.

## O que registra

- Configurações: `counter.github.host`, `counter.github.include_drafts`,
  `counter.github.label_filter`, `counter.gitlab.host`, `counter.linear.team`,
  `counter.jira.project`, `counter.poll.interval_s`, `counter.notifications.new_request`,
  `counter.notifications.checks_failed`.
- Comando `counter.sources`: chama `describe` de cada `counter_<nome>` instalado e devolve
  `{counters: [{counter, module, label_key}]}`. Não faz rede.

## Contratos

Não publica eventos. Os balcões leem as configurações com `cx.setting(...)`, que funciona
mesmo sem este módulo (cada balcão tem o padrão dele).

As mensagens de erro de balcão ficam em cada módulo que as mostra (`<módulo>-error-*`),
porque as chaves i18n são globais e cada módulo precisa funcionar sozinho.

## Como testar

```sh
cargo test -p forja-module-counter
```

## Lote 3 (cobertura do catálogo)

- `counter.poll.interval_s` (`poll.rs`): view modal que nunca abre e pede
  `request.refresh` no intervalo, enquanto houver balcão instalado.
- `counter.notifications.new_request` e `counter.notifications.checks_failed`
  (`notify.rs`): avisam pedido novo e checks que passaram a falhar. O primeiro resultado
  de cada balcão só registra o estado. Balcões GitHub e GitLab ainda não preenchem
  `RequestItem.checks`.
