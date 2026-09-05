# Atualização automática do MasterNote

O app checa o gateway de atualizações, e quando há versão nova mostra um toast
no canto inferior direito com **Depois** e **Atualizar**. Nada é baixado antes
do clique.

## Como funciona

| Peça | Onde |
| --- | --- |
| Plugins Rust | `tauri-plugin-updater` + `tauri-plugin-process` em `src-tauri/Cargo.toml`, registrados em `src-tauri/src/lib.rs` |
| Endpoint e chave pública | `plugins.updater` em `src-tauri/tauri.conf.json` |
| Permissões (ACL) | `src-tauri/capabilities/updater.json` — só a janela `main` |
| Estado e checagem | `frontend/src/update/state.ts`, `frontend/src/update/useUpdate.ts` |
| Toast | `frontend/src/components/UpdateToast.tsx` |
| Publicação | `scripts/release.ps1` + `deploy.toml` |
| Rota do gateway | `deploy-base/gateway/src/index.js`, função `updaterJson` |

Ritmo da checagem: 20 s depois de abrir (para não disputar rede com o login e a
primeira sincronização) e a cada 6 h enquanto o app fica aberto.

Falha de checagem é **silenciosa** de propósito — máquina sem internet ou atrás
de proxy não deve ver toast vermelho a cada 6 h por algo que não pode resolver.
Erro só aparece depois que o usuário clicou "Atualizar".

No Windows o `installMode` é `passive`: o instalador roda com barra de
progresso, sem perguntas. O app **não** reinicia sozinho — o toast passa a
"Reiniciar agora" e espera.

## O caminho até a máquina do usuário

Uma release do MasterNote carrega **dois manifestos**, porque tem dois públicos:

| Arquivo | Assinatura | Quem lê |
| --- | --- | --- |
| `masternote.stable.manifest.json` | Ed25519 (`C:/chaves/deploy-base.pem`) | o **portal**, para montar o botão de download |
| `latest.json` | minisign (`C:/chaves/masternote-updater.key`) | o **app instalado**, para se atualizar |

São mecanismos independentes que convivem na mesma release, e `scripts/release.ps1`
publica os dois juntos — publicar só um deixa metade do caminho funcionando em
silêncio: ou o portal sem botão, ou o app sem atualização.

O app não fala com o GitHub. Ele fala com o gateway:

```
app  ──GET──▶  /v1/apps/masternote/updater.json   (Cloudflare Worker)
                        │  busca o latest.json da release, com token do servidor,
                        │  e reescreve cada `url` para a rota /download dele mesmo
                        ▼
                ShinuuL/Releases  (privado)  tag masternote-v0.2.0
```

Isso existe porque **repositório privado não serve direto**: o updater faz a
requisição do lado Rust e não manda credencial nenhuma, então um `latest.json`
atrás de autenticação volta 404 para ele — e o toast simplesmente nunca aparece,
sem erro visível, porque falha de checagem é silenciosa por design.

A reescrita das URLs é a outra metade do problema: o `latest.json` traz links
diretos do GitHub, que num repo privado também são fechados. A rota troca cada
um pelo `/v1/apps/masternote/download/:version/:file` do próprio gateway.

**A assinatura não é tocada nessa reescrita** — ela cobre o conteúdo do `.zip`,
não a URL. O gateway continua sendo só transporte: se ele mentir sobre o
endereço, o cliente baixa e rejeita na verificação minisign.

## Chave de assinatura do updater

- privada — `C:/chaves/masternote-updater.key`, **fora do repositório**
- pública — `...key.pub`, colada no `pubkey` do `tauri.conf.json`

> **Histórico (2026-09-05).** A chave original foi gerada em 2026-09-04 em
> `C:\Users\Mastersys\.masterdesk\masternote-updater.key` — em outra máquina,
> inacessível daqui. Como nenhuma release chegou a ser publicada com ela e não
> há instalação em campo carregando aquele `pubkey`, foi gerado um par novo em
> `C:/chaves/` (junto do `deploy-base.pem`, mesma convenção do ecossistema).
> Se preferir voltar à chave original, basta restaurá-la e desfazer a troca do
> `pubkey` no `tauri.conf.json` — enquanto não houver release publicada, os dois
> caminhos são equivalentes.

**Faça uma cópia de segurança dela agora.** Perder a chave privada não é um
contratempo: as instalações existentes só aceitam pacotes assinados por ela, e
sem ela nenhuma versão futura atualiza ninguém — cada máquina teria de ser
reinstalada à mão com um `pubkey` novo.

A chave foi gerada **sem senha**, seguindo o que já vale para o
`deploy-base.pem`. `scripts/release.ps1` a lê do disco na hora do build e limpa
a variável de ambiente depois.

## Publicando uma versão

1. Subir a versão nos **três** arquivos (`Cargo.toml` do workspace,
   `src-tauri/tauri.conf.json`, `frontend/package.json`) — os três precisam
   bater. O script recusa publicar se divergirem: a versão do
   `tauri.conf.json` vai compilada dentro do binário, e uma tag adiantada faz
   cada instalação se achar desatualizada contra ela mesma, mostrando o toast
   em looping sem nunca convergir.
2. Commit e push.
3. `pwsh scripts/release.ps1 -Version 0.3.0 -NotesFile CHANGELOG-0.3.0.md`

O script compila, assina, cria a release `masternote-v0.3.0` em
`ShinuuL/Releases` e anexa os dois manifestos. Use `-DryRun` para compilar e
inspecionar o manifesto assinado sem enviar nada.

**Publicar é o gesto que dispara a atualização em todas as máquinas** — o
`deploy-base` cria a release como rascunho, sobe os assets e só então publica,
para o gateway nunca enxergar uma release pela metade.

Conferindo depois:

```bash
curl https://updates-gateway.sofaltaumaletr.workers.dev/v1/apps/masternote/updater.json
curl https://updates-gateway.sofaltaumaletr.workers.dev/v1/apps/masternote/latest
```

> **Pré-release não atualiza ninguém.** A rota `/updater.json` filtra
> `prerelease`, porque o updater do Tauri não tem canal — uma beta ali viraria
> atualização automática para todo mundo. Publicar com `--channel beta` serve
> para o portal, não para o updater.

Para conferir de ponta a ponta: instale a versão anterior numa máquina, publique
a nova, e o toast deve aparecer em até 20 s ao abrir o app.

## O que o CI faz (e não faz)

`.github/workflows/release.yml` só **confere que compila** e guarda o instalador
como artefato por 7 dias. Ele não publica.

O motivo é a chave: a Ed25519 do deploy-base por decisão de projeto nunca sai da
máquina que publica — é justamente o que faz o gateway e o CDN serem só
transporte. Colocá-la num secret do Actions desfaria a propriedade que ela
existe para garantir.

> Até 2026-09-05 esse workflow disparava em tags `v*` e publicava com a
> `tauri-action` no próprio `ShinuuL/MasterDesk`. Isso era incompatível com a
> decisão de 2026-09-04 em dois pontos: publicava no repo público errado, e
> usava a tag `v0.2.0`, que não casa com o padrão `<app>-v<semver>` pelo qual o
> gateway descobre os apps — o portal nunca enxergaria a release.

## Pendências

### A primeira instalação ainda é manual

O updater só alcança quem **já tem** uma versão com estes plugins dentro. A
0.1.0 e a 0.2.0 distribuídas hoje não têm. Então:

- a versão que sair com isto ainda precisa ser instalada à mão, uma vez;
- a `docs/PENDENCIAS.md` já pede desinstalar o "MasterDesk" antigo antes — vale
  aproveitar a mesma rodada;
- da próxima em diante, o toast resolve.

### SmartScreen continua

Assinatura de código foi dispensada pelo DEV em 2026-09-02. A assinatura do
updater (minisign) é outra coisa: garante ao app que o pacote veio de nós, mas
não fala com o Windows. O SmartScreen segue aparecendo — inclusive durante a
atualização automática, se ele decidir intervir no instalador.
