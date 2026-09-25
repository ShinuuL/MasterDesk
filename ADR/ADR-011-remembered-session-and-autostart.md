# ADR-011 — Sessão lembrada ("manter conectado") e iniciar com o sistema

**Status:** Proposto (2026-09-25) — implementado, **aguarda compilação/testes Rust
e validação no Windows** (a máquina em que foi escrito não tinha o linker MSVC).

## Contexto

Pedido do DEV: o usuário não deveria digitar a senha da conta local a cada
abertura, e o app deveria poder abrir junto com o Windows.

Até aqui a sessão local vivia só em memória (`LocalAuthRepository`), então todo
início pedia login. A sessão do **Mastersys** já sobrevivia (refresh token no
cofre, ADR-006) — o que faltava era a conta local.

## Opções — sessão lembrada

1. **Guardar o `user_id` no banco.** Rejeitado: o SQLite fica sem criptografia
   em `%APPDATA%`; qualquer cópia do arquivo "loga" como qualquer usuário.
2. **Guardar o `user_id` no cofre do SO.** Funcionaria, mas não tem expiração
   nem revogação independente do cofre.
3. **Token aleatório no cofre + hash no banco (escolhida).** O cofre (Credential
   Manager no Windows, via `keyring` 3.x já adotado) guarda `<id>.<segredo>`;
   o banco guarda só o hash Argon2id do segredo em `remembered_sessions`.

## Decisão — sessão lembrada

- Segredo de 256 bits do `OsRng`; formato `<uuid>.<64 hex>`.
- Uma sessão lembrada por usuário do SO (um único item no cofre); lembrar de
  novo substitui a anterior.
- Validade **deslizante de 30 dias** (`REMEMBER_FOR_DAYS`), renovada a cada
  abertura.
- Rejeitado e limpo (banco + cofre): token malformado, sem linha no banco,
  expirado ou com segredo que não confere.
- **O banco é a revogação que vale.** `Sair` apaga a linha (erro sobe para a UI)
  e tenta apagar o cofre (falha do cofre não bloqueia o logout, porque o token
  que sobrar não autentica sem a linha).
- Login com a caixa **desmarcada** também apaga uma sessão lembrada anterior.
- Cofre indisponível (ex. Linux sem Secret Service): o login vale, a sessão não
  é lembrada, e a UI mostra aviso. Nunca há fallback para texto plano.
- Caixa ligada por padrão (padrão de apps desktop; o cofre é por usuário do SO).

Port `AuthenticationProvider` ganhou `remember_session`, `restore_session` e
`forget_remembered_session`. Único implementador real: `LocalAuthRepository`.

## Opções — iniciar com o sistema

1. **`tauri-plugin-autostart` (escolhida).** Oficial (tauri-apps/plugins-workspace),
   2.5.1 estável, MIT OU Apache-2.0, Windows/macOS/Linux. No Windows grava em
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` via `auto-launch` 0.5 —
   por usuário, sem administrador.
2. **Escrever no registro direto (`winreg`).** Menos uma camada, mas só Windows e
   reimplementa o que o plugin já trata (override do Gerenciador de Tarefas).
3. **Atalho na pasta Inicializar.** Depende do instalador e não é controlável
   pela UI de forma limpa.

## Decisão — iniciar com o sistema

- Controlado só por comandos Rust (`src-tauri/src/autostart.rs`); **nenhuma**
  permissão `autostart:*` é dada ao JavaScript.
- O estado mostrado é o lido do SO após cada operação, não uma cópia no banco.
- Verificado no fonte (`auto-launch-0.5.0/src/windows.rs`): `is_enabled` respeita
  o desligamento feito pelo Gerenciador de Tarefas (`StartupApproved\Run`), e
  `enable` o reativa. `disable` falha se o valor não existe — por isso o
  comando só age quando o estado muda.
- O app abre normalmente (janela visível) quando iniciado pelo Windows.

## Consequências

- Nova migration `0011_remembered_sessions.sql`.
- Nova dependência `tauri-plugin-autostart` (+ `auto-launch`, `winreg` 0.10,
  `dirs` 4 transitivas).
- Em `tauri dev`, ligar o autostart registra o executável de **debug**; desligue
  antes de instalar a versão de release.
- A proteção da conta local continua sendo de interface: os comandos de dados
  não conferem sessão (achado registrado na auditoria de 2026-09-25).
- Pendente de validação: compilar e rodar `cargo test` (10 testes novos em
  `local_auth_repository.rs`, 3 em `auth.rs`), testar no Windows ligar/desligar,
  reiniciar e conferir o Gerenciador de Tarefas → Aplicativos de inicialização.
  macOS/Linux não validados.
