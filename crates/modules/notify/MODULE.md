# forja-module-notify

Avisos dentro da janela e notificações do Windows.

## O que registra

- comando `notify.show` e o método RPC de mesmo nome (`title`, `body?`, `level?`, `bench_path?`);
- comando `notify.recent`, que lista o centro;
- evento `notify.requested` (`NotifyRequested`), publicado por qualquer módulo e consumido aqui;
- configurações `notifications.*` do catálogo (o módulo âncora `notifications` existe no shell para o prefixo da chave);
- vista `notify.toasts` no slot modal e `notify.center` na barra de status.

## Regras

- Com a janela em foco e `notifications.os.only_unfocused`, não há notificação do Windows nem som.
- O horário de silêncio (`notifications.quiet.*`) corta toast, Windows e som. O intervalo pode cruzar a meia-noite.
- O som é um WAV gerado na hora, com ganho 0,4 nas amostras (`sound::OWN_GAIN`). Não há chamada de mixer nem mudança do volume do sistema.
- A notificação do Windows usa o AppUserModelID `Forja.App`, definido pelo shell no processo.

## Como testar

```
cargo test -p forja-module-notify
```
