# voice

Voz da Forja: ditado pelo Windows (Win+H) pela tecla do keymap, gravação local pelo microfone
padrão com medidor, transcrição opcional pela OpenAI com a chave no Gerenciador de Credenciais
do Windows, e inserção no terminal, no editor e no chat, sempre depois de confirmação.

## Arquivos

| Caminho | Papel |
|---|---|
| `src/gesture.rs` | gesto da tecla: alternar (um toque liga, outro desliga) ou segurar (liga ao apertar, desliga ao soltar); função pura |
| `src/wav.rs` | medidor de nível (RMS), conversão para 16 bits, mistura de canais, WAV pelo `hound` |
| `src/multipart.rs` | corpo `multipart/form-data` com campos de texto e arquivo; testado contra o fixture |
| `src/openai.rs` | envio ao endpoint de transcrição e leitura da resposta; erros viram chaves de tradução |
| `src/keystore.rs` | cofre da chave: `keyring` no Windows; trait para os testes |
| `src/winkeys.rs` | sequência Win+H e envio pelo `SendInput`; trait para os testes |
| `src/record.rs` | gravação pelo `cpal` numa thread própria; nível num contador atômico |
| `src/state.rs` | estado compartilhado, backends injetáveis, configurações lidas |
| `src/actions.rs` | ações: ditado, gravar, transcrever, chave, terminais, inserir; disco e rede em `Ctx::spawn` |
| `src/commands.rs` | comandos de voz; a inserção exige `confirmed: true` |
| `src/view.rs` | janela de gravação (camada `Modal`) e seção "Voz" dos Ajustes |
| `src/tests.rs` | contrato do kernel e comportamento com backends falsos |
| `tests/fixtures/transcription_body.txt` | corpo multipart esperado, com quebras LF (o teste converte para CRLF) |

## O que registra

- Camada `Modal` `voice.overlay` (ordem 20): janela de gravação ancorada no canto inferior direito.
- Seção de Ajustes `voice.settings` (ordem 25, depois de "Ajudantes", porque o catálogo põe as chaves de voz no grupo Ajudantes).
- Comandos: `voice.dictate`, `voice.hold`, `voice.record.start`, `voice.record.stop`, `voice.transcribe`, `voice.insert`, `voice.discard`, `voice.key.save`, `voice.key.delete`, `voice.targets.refresh`.
- Configurações: `voice.mode.enabled`, `voice.input.engine`, `voice.input.language`, `voice.privacy.keep_audio`.
- Atalho `Ctrl+Alt+V` para `voice.dictate` (`keymap.toml`). O id ainda não consta em `design/keymap.toml`: pendência para o coordenador.
- Bundles `i18n/en.ftl` e `i18n/pt-BR.ftl`.
- Não há método RPC.

## Contratos

Consome `terminal.list` e `terminal.send` (terminal), e `editor.insert` e `chat.insert` quando existirem. Sem eles, o destino aparece indisponível. Não publica nem consome eventos. Formas e regras: `design/contracts.md`, seção "Voz (onda C2)".

## Regras

- **Desligada por padrão.** Ditado e gravação exigem `voice.mode.enabled`.
- **Ditado.** Win+H (`LWIN` desce, `H` desce e sobe, `LWIN` sobe). O estado "ligado" é aproximado: o Windows pode encerrar o ditado sozinho, e o módulo não sabe.
- **Gravação.** O stream do `cpal` vive numa thread própria. Limite de 5 minutos de áudio. Sem `keep_audio`, o WAV fica na memória e some.
- **Transcrição.** Só por pedido explícito. Envia a `https://api.openai.com/v1/audio/transcriptions` com o campo `model`, o idioma (`pt` ou `en`, ou nada para detectar) e o arquivo WAV.
- **Chave.** Guardada em `forja.voice` / `openai` no cofre. Não volta em resposta nem em aviso. A tela é um campo de senha.
- **Erros.** Viram chaves `voice-error-*`: autenticação, HTTP, rede, tempo limite, formato, cofre, microfone.
- **Inserção.** Só com `confirmed: true`; a tela mostra o destino e o texto antes.
- **`whisper_cpp`.** Aparece no catálogo e em Ajustes, mas não está implementado: responde `voice-error-engine`.

## Limitações conhecidas

- Sem `whisper_cpp` e sem a parte de síntese de voz (`voice.output.*`): fora do escopo desta entrega.
- `voice.input.model_path` não é registrado: só vale para o `whisper_cpp`.
- O ditado do Windows é disparado pela tecla e pelo comando, não por um atalho de sistema; o estado mostrado é aproximado.
- Editor e chat só entram quando os comandos `editor.insert` e `chat.insert` existirem; hoje não existem neste repositório.
- Nenhum teste abre o microfone nem envia tecla de verdade; a captura de áudio foi verificada só por leitura do código.

## Como testar

```sh
cargo test -p forja-module-voice
FORJA_FIXTURES=1 FORJA_FIXTURES_VOICE=1 cargo run -p forja-app -- --screenshot shots/voice-overlay.png --slot all
```

`FORJA_FIXTURES_VOICE=1` abre a janela de voz com dados de exemplo; sem ele, a janela fica fechada
nas capturas. Os testes usam uma chave em memória e um teclado que só registra os passos.

## Lote 3 (adiado)

- Adiadas no catálogo: `voice.input.model_path`, `voice.output.engine`, `voice.output.model_path`,
  `voice.output.rate` e `voice.output.auto_read`. Dependem de síntese de voz (SAPI ou Piper)
  e do motor whisper.cpp, que não estão no workspace. Ficam adiados até haver dependência aprovada.
