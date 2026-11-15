# AGENTS.md — como trabalhar na Forja

Este arquivo vale para pessoas e para modelos. Leia-o inteiro antes de mudar qualquer coisa.

## 1. Regras de autoria (obrigatórias)
- A Forja é **100% autoral**. Conceitos, nomes, textos, padrões, atalhos, ids, chaves de configuração, temas e skills vêm de `design/` e de decisões nossas.
- **Proibido consultar ou copiar** de outros produtos de mesmo tipo: código, dados, textos, ids, chaves, constantes ou estrutura de navegação.
- **Permitido** reaproveitar código-fonte que nós mesmos escrevemos, desde que fique atrás das APIs da Forja, com nomes, ids, chaves e dados da Forja.
- Fatos sobre ferramentas de terceiros (por exemplo, como iniciar o CLI `claude`) vêm da **documentação oficial ou do `--help` delas**, com a fonte citada no arquivo.
- `cargo xtask provenance` precisa passar. Ele recusa termos e identificadores da lista de proveniência.

## 2. Mapa do repositório
| Caminho | O que é |
|---|---|
| `design/` | Fonte de verdade do produto: `product.md` (conceitos e vocabulário), `brand.md`, `settings.md`, `keymap.md`, `ui.md`, `roadmap.md` (ordem de trabalho das melhorias) |
| `crates/kernel` | Núcleo: contrato `Module`, `Registrar`, comandos, eventos, tarefas, armazenamento, configurações, atalhos e i18n. Não depende de egui. |
| `crates/protocol` | Tipos do RPC/CLI/MCP (serde) e o schema de métodos |
| `crates/ui-kit` | Design system (tokens "Brasa" e componentes egui) |
| `crates/shell` | O app: janela, slots de layout, carregamento dos módulos |
| `crates/cli` | CLI `forja`, gerado a partir do schema do `protocol` |
| `crates/hook` | `forja-hook`, que repassa eventos de status dos agentes |
| `crates/modules/<nome>` | **Uma funcionalidade = um crate.** Se registra sozinho. |
| `content/` | Conteúdo autoral embutido: perfis de ajudantes, temas, atalhos, skills, modelos |
| `extensions/` | Extensões de exemplo (Lua + arquivos) |
| `xtask/` | `cargo xtask new-module`, `new-extension`, `provenance`, `catalog`, `package`, `installer`, `atlas` |
| `atlas/` | Atlas visual: agente que documenta cada tela (`atlas/README.md`); resultados no branch `atlas` |
| `installer/` | Roteiro do instalador (Inno Setup) |
| `playtest/` | Testadores automáticos: agentes que usam o app ao vivo (`forja-app --test-driver` + `forja-mcp --player`) |

## 3. Receitas
### Criar uma funcionalidade (módulo Rust)
1. `cargo xtask new-module <nome>`. Isso cria `crates/modules/<nome>/` com `Cargo.toml`, `src/lib.rs`, `MODULE.md`, `i18n/en.ftl`, `i18n/pt-BR.ftl`, `keymap.toml` e testes, e já faz o vínculo com o shell.
2. Implemente `register()`: comandos, views em slots, configurações, atalhos, eventos e métodos RPC.
3. Não edite nenhum arquivo fora da pasta do seu módulo. Se precisar de algo do núcleo, proponha a mudança no kernel numa tarefa separada.

### Criar uma extensão (sem compilar)
`cargo xtask new-extension <nome>` cria `extensions/<nome>/extension.toml` + `init.lua`. Consulte `docs/extensions.md`.

### Adicionar conteúdo
Um comando é um `.md` em `content/commands/`. Um perfil de ajudante é um `.toml` em `content/helpers/`. Um tema é um `.toml` em `content/themes/`. O mesmo formato vale em `~/.forja/` e em `<projeto>/.forja/`.

## 4. Estilo e qualidade
- **Todo identificador é em inglês:** código Rust, API Lua (funções e campos), ids, chaves de configuração, chaves de TOML/frontmatter e nomes das pastas de conteúdo (`commands/`, `helpers/`, `skills/`, `themes/`, `keymaps/`, `recipes/`, `templates/`). O português aparece só em textos de interface (sempre via i18n pt-BR + en, nunca strings literais na UI) e na documentação.
- Arquivos com no máximo ~600 linhas e módulos com no máximo ~5k linhas. API pública documentada no `lib.rs`, e cada crate tem um `MODULE.md`: propósito, o que registra, contratos e como testar.
- Sem trabalho de disco, rede ou processo na thread da interface. Use `Tasks` do kernel.
- Nunca escreva fora das pastas da Forja (configs de outros apps, por exemplo) sem consentimento explícito do usuário registrado em configuração. Testes e execuções isoladas nunca tocam arquivos globais.
- Antes de entregar: `cargo build`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo xtask provenance` e `cargo xtask catalog`.
- Dependências: só as que estão em `[workspace.dependencies]` do `Cargo.toml` raiz. Pedir uma nova é tarefa do coordenador.
