# skill

Skills da Forja e dos ajudantes: uma skill é uma pasta com `SKILL.md`. O módulo descobre as
skills nas pastas de cada dono e escopo, mostra o front matter e os erros, instala de uma pasta ou
de um zip, copia para outro ajudante e manda para a lixeira. A página "Skills" fica no trilho
(`Slot::Rail`, com o glifo `SKILLS`).

## Arquivos

| Caminho | Papel |
|---|---|
| `src/frontmatter.rs` | front matter (`---` ... `---`): `name` e `description`, valores entre aspas, blocos `>` e `|`, regra do nome |
| `src/model.rs` | escopo, dono, raiz, skill listada, aviso e erro com chave |
| `src/sources.rs` | pastas de cada ajudante (`[skills]` dos perfis embutidos) e montagem das raízes; funções puras |
| `src/discovery.rs` | leitura das raízes: pasta sem `SKILL.md` não é skill; pasta oculta é ignorada |
| `src/install.rs` | instalar pasta e zip, copiar, remover; pasta temporária ao lado do destino; zip conferido entrada por entrada |
| `src/actions.rs` | ações com efeito: leitura, instalar, copiar, remover, `SKILL.md` do item, consentimento; disco em `Ctx::spawn` |
| `src/commands.rs` | comandos e o RPC `skill.install`; validação dos argumentos |
| `src/state.rs` | estado compartilhado: última leitura, operação pendente, aviso, conflito |
| `src/view.rs` | a página "Skills": lista por dono e escopo, busca, detalhe com o `SKILL.md`, confirmações |
| `src/fixtures.rs` | dados de captura (`FORJA_FIXTURES=1`), sem disco; escritas recusadas |
| `src/tests.rs` | contrato do kernel, recusas síncronas e a forma da foto |

## O que registra

- Página `skill.page` no trilho (`Slot::Rail`, ordem 60, glifo `SKILLS`), título "Skills".
- Comandos: `skill.refresh`, `skill.install`, `skill.copy`, `skill.remove`.
- Métodos RPC/CLI: `skill.list` (read; começa a leitura se ainda não houve), `skill.install` (write).
- Bundles `i18n/en.ftl` e `i18n/pt-BR.ftl`. `keymap.toml` só com comentário: a página abre pela barra do trilho.

## Contratos

Consome `bench.current` (pasta do projeto, para as skills de projeto). Não publica nem consome
eventos. Formas de JSON, erros e regras: `design/contracts.md`, seção "Skills (onda C2)".

As pastas de cada ajudante vêm dos perfis em `content/helpers` (bloco `[skills]`), embutidos
pelo `include_str!`, como o módulo `helper` faz. Perfis sem fonte oficial confirmada têm as listas
vazias e o comentário "a confirmar".

## Regras

- **Front matter.** `name` e `description` são obrigatórios. `name` segue a regra do formato:
  1 a 64 caracteres, só minúsculas, dígitos e hífens, sem hífen no começo, no fim ou duplo. Erro
  de front matter não impede a listagem: a skill aparece com o motivo.
- **Sobrescrever.** Se a skill já existe, a operação devolve `conflict` e não altera nada. Com
  `overwrite`, a versão antiga vai para a lixeira (`trash`), nunca apagada direto.
- **Consentimento.** Escrever numa raiz que não é da Forja exige `consent`. A tela só envia isso
  depois da confirmação, com o caminho à vista. O consentimento vai para `<dados>/modules/skill.json`.
- **Zip.** Caminho que sai da pasta (`..`, absoluto, letra de unidade, `:`) ou link simbólico
  recusa a instalação inteira. A extração fica numa pasta temporária, apagada no fim.
- **Limites.** Até 2000 arquivos e 64 MB por instalação; 500 skills por raiz; `SKILL.md` de até 256 KB.
- **Remoção.** Só para pastas listadas que têm `SKILL.md`. Vai para a lixeira.
- **Sem disco na thread da interface.** Ler, copiar, remover e ler o `SKILL.md` rodam em `Ctx::spawn`.

## Limitações conhecidas

- O `SKILL.md` aparece como texto puro, sem renderizar Markdown: o visualizador de Markdown ainda não existe.
- A validação segue a regra de nome e a presença de `name` e `description`. O limite de tamanho da
  descrição não foi aplicado, porque só havia fonte secundária para ele.
- A integridade do zip (CRC) é a que a biblioteca `zip` verifica; o módulo confere os caminhos das entradas e o tamanho extraído.

## Como testar

```sh
cargo test -p forja-module-skill
FORJA_FIXTURES=1 cargo run -p forja-app -- --screenshot shots/skills-page.png --slot bench
```

Os testes usam pastas temporárias e uma lixeira falsa. Nenhum teste lê as pastas reais de
ajudantes, nem chama `skill.list` (que começaria uma leitura do disco).
