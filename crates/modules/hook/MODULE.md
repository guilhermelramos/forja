# forja-module-hook

Ponte mínima entre o binário `forja-hook` e o módulo `helper`.

## O que registra

- Método RPC `hook.event` com o parâmetro `payload` (JSON).

O binário lê o stdin e chama `hook.event`. Este módulo repassa o payload ao
comando `helper.hook`; sem o módulo helper, o evento é um no-op silencioso e o
hook continua saindo com código 0.

## Por que um módulo separado

O kernel exige que o nome de um método RPC comece pelo id do módulo que o
registra; `hook.event` não poderia ser registrado pelo módulo `helper`. A ponte
mantém o nome que o binário já usa e deixa o helper dono do status.

## Como testar

```sh
cargo test -p forja-module-hook
echo '{"hook_event_name":"Stop"}' | target/debug/forja-hook.exe
```
