# forja-module-onboarding

Primeiro uso da Forja. O cartão abre sozinho enquanto não for concluído, e de novo pelo comando `onboarding.open` ou pela seção de Ajustes.

## O que registra

- comando `onboarding.open`;
- vista modal `onboarding.wizard` e seção `onboarding.settings`;
- configuração `onboarding.projects.dir` (caminho da pasta de projetos).

## Passos

Idioma (`i18n.language.mode`), tema (`appearance.theme.mode`), pasta de projetos, importação do Harbor (`import.harbor.*`, depois da pasta; aparece só se o módulo `import` existe e há itens pendentes), detecção de ajudantes (`helper.detect`, pulada se o comando não existe), avisos (`notifications.toast.enabled` e `notifications.os.enabled`) e uma checklist. Ao concluir, tenta `notify.show`.

`FORJA_SCREENSHOT=1` impede a abertura automática, para os screenshots do shell não ficarem cobertos. `--open onboarding` chama o comando e força o cartão.

`FORJA_ONBOARDING_STEP=<índice>` abre o cartão direto num passo, pelo índice da sequência (captura de tela; `3` é o passo de importação quando ele aparece).

## Importação do Harbor (módulo `import`)

A página de importação é só interface: ela lê `import.harbor.status`, chama `import.harbor.preview` ao carregar o cartão, e chama `import.harbor.run` (com os ids marcados) ou `import.harbor.skip`. Toda a lógica fica no módulo `import`. Sem esse módulo, o passo some.

## Como testar

```
cargo test -p forja-module-onboarding
```
