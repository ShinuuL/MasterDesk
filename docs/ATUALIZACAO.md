# Atualização automática do MasterNote

O app checa o GitHub Releases, e quando há versão nova mostra um toast no canto
inferior direito com **Depois** e **Atualizar**. Nada é baixado antes do clique.

## Como funciona

| Peça | Onde |
| --- | --- |
| Plugins Rust | `tauri-plugin-updater` + `tauri-plugin-process` em `src-tauri/Cargo.toml`, registrados em `src-tauri/src/lib.rs` |
| Endpoint e chave pública | `plugins.updater` em `src-tauri/tauri.conf.json` |
| Permissões (ACL) | `src-tauri/capabilities/updater.json` — só a janela `main` |
| Estado e checagem | `frontend/src/update/state.ts`, `frontend/src/update/useUpdate.ts` |
| Toast | `frontend/src/components/UpdateToast.tsx` |
| Publicação | `.github/workflows/release.yml` |

Ritmo da checagem: 20 s depois de abrir (para não disputar rede com o login e a
primeira sincronização) e a cada 6 h enquanto o app fica aberto.

Falha de checagem é **silenciosa** de propósito — máquina sem internet ou atrás
de proxy não deve ver toast vermelho a cada 6 h por algo que não pode resolver.
Erro só aparece depois que o usuário clicou "Atualizar".

No Windows o `installMode` é `passive`: o instalador roda com barra de
progresso, sem perguntas. O app **não** reinicia sozinho — o toast passa a
"Reiniciar agora" e espera.

## Pendências antes de isto funcionar

### 1. Endereço do manifesto — depende do gateway

O `tauri.conf.json` aponta hoje para as releases do próprio repositório de
código:

```
https://github.com/ShinuuL/MasterDesk/releases/latest/download/latest.json
```

Isso funciona **se** as releases forem publicadas aqui e forem públicas. A
decisão do DEV em 2026-09-04 foi outra: os instaladores ficam num **repositório
de releases privado**, com um **gateway** a configurar depois.

Duas coisas seguem disso, e a primeira é a que trava:

- **Repositório privado não serve direto.** O updater faz a requisição do lado
  Rust e não manda credencial nenhuma — um `latest.json` atrás de autenticação
  volta 404 para ele, e o toast simplesmente nunca aparece. Não há erro visível,
  porque falha de checagem é silenciosa por decisão de design. É exatamente para
  isso que o gateway existe: um endereço público que busca do repo privado com
  token do lado do servidor.
- **Quando o gateway existir, trocar a linha acima por ele.** O updater aceita
  as variáveis `{{target}}`, `{{arch}}` e `{{current_version}}` no endereço, se
  o gateway preferir rotear por elas — por exemplo
  `https://updates.mastersys.app.br/masternote/{{target}}/{{current_version}}`.
  Um `latest.json` único também basta; a forma é escolha do gateway.

O gateway precisa devolver o JSON **e** deixar o `.msi.zip`/`.nsis.zip` que ele
referencia acessível pela URL que está lá dentro — o `latest.json` gerado pelo
workflow traz links do GitHub, que num repo privado também são fechados. Ou o
gateway reescreve essas URLs para si mesmo, ou serve os dois pelo mesmo caminho.

### 1b. Onde o workflow publica

`.github/workflows/release.yml` roda em `ShinuuL/MasterDesk` e a `tauri-action`
publica a release **no repositório onde roda**. Para os artefatos irem parar no
repo de releases privado, é preciso um passo a mais: um `gh release upload`
apontando para o outro repo, com um PAT em secret (o `GITHUB_TOKEN` do Actions
não alcança outro repositório).

Enquanto o gateway não existir, o caminho mais curto para testar ponta a ponta é
publicar uma release **pública** aqui mesmo — a URL já configurada funciona sem
nenhuma alteração.

### 2. Chave de assinatura

Gerada em 2026-09-04 e **fora do repositório**:

- privada — `C:\Users\Mastersys\.masterdesk\masternote-updater.key`
- pública — `...key.pub`, já colada no `pubkey` do `tauri.conf.json`

A privada precisa virar o secret `TAURI_SIGNING_PRIVATE_KEY` no GitHub (e
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` vazio, porque foi gerada sem senha).

**Faça uma cópia de segurança dela agora.** Perder a chave privada não é um
contratempo: as instalações existentes só aceitam pacotes assinados por ela, e
sem ela nenhuma versão futura atualiza ninguém — cada máquina teria de ser
reinstalada à mão com um `pubkey` novo.

### 3. A primeira instalação ainda é manual

O updater só alcança quem **já tem** uma versão com estes plugins dentro. A
0.1.0 e a 0.2.0 distribuídas hoje não têm. Então:

- a versão que sair com isto ainda precisa ser instalada à mão, uma vez;
- a `docs/PENDENCIAS.md` já pede desinstalar o "MasterDesk" antigo antes — vale
  aproveitar a mesma rodada;
- da próxima em diante, o toast resolve.

### 4. SmartScreen continua

Assinatura de código foi dispensada pelo DEV em 2026-09-02. A assinatura do
updater (minisign) é outra coisa: garante ao app que o pacote veio de nós, mas
não fala com o Windows. O SmartScreen segue aparecendo — inclusive durante a
atualização automática, se ele decidir intervir no instalador.

## Publicando uma versão

1. Subir a versão nos **três** arquivos (`Cargo.toml` do workspace,
   `src-tauri/tauri.conf.json`, `frontend/package.json`) — os três precisam
   bater.
2. Commit, e então `git tag v0.3.0 && git push --tags`.
3. O workflow compila e cria uma release **rascunho** com o instalador, o
   `.sig` e o `latest.json`.
4. Revisar o texto — a primeira frase do corpo é o que aparece dentro do toast.
5. **Publicar a release.** Só aí o updater passa a enxergá-la, e é esse gesto
   que dispara a atualização em todas as máquinas.

Para conferir antes de publicar de verdade: instale a versão anterior numa
máquina, publique a nova, e o toast deve aparecer em até 20 s ao abrir o app.
