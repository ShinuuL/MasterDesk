# MasterDesk ↔ Mastersys Suporte — apontando para produção

> **Status:** validado ponta a ponta nos dois ambientes — primeiro contra o
> local (`http://localhost:3000`) e depois **contra produção**
> (`https://mastersys.app.br/suporte`), em 02/09/2026. O endereço da seção 1
> funciona como descrito. A seção 4 fica como checklist de diagnóstico caso a
> sincronização falhe para algum usuário.

---

## 1. O endereço que o usuário digita

Em **Tarefas → Mastersys → Endereço**:

```
https://mastersys.app.br/suporte
```

Sem `/api` no final e sem barra final — o provider concatena os caminhos. O
resultado é exatamente o que o frontend web já usa hoje:

| O que o app chama | URL final |
|---|---|
| `POST /api/auth/login` | `https://mastersys.app.br/suporte/api/auth/login` |
| `POST /api/auth/refresh` | `https://mastersys.app.br/suporte/api/auth/refresh` |
| `GET /api/tasks/users/:id` | `https://mastersys.app.br/suporte/api/tasks/users/:id` |
| `GET /api/tickets/paginated?assignedTo=…` | `https://mastersys.app.br/suporte/api/tickets/paginated?…` |

Isso bate com `frontend/.env.android` (`VITE_API_URL=https://mastersys.app.br/suporte/api`)
e com `frontend/.env.production` (`/suporte/api`, relativo ao mesmo domínio).

Credenciais: o **mesmo usuário e senha do Mastersys**. Não há usuário técnico
nem chave de API separada nesta integração.

---

## 2. Exposição: o que o cliente precisa conhecer

O usuário conhece apenas o **domínio público do site** — o mesmo que ele já
digita no navegador para usar o sistema. Host interno, porta 3000 e a topologia
do servidor continuam invisíveis atrás do reverse proxy.

Em termos de superfície exposta, configurar o MasterDesk **não acrescenta nada**
ao que já está público quando alguém abre o sistema no navegador.

### Por que não existe webhook — e por que ele não ajudaria

A integração é estritamente **pull**: o MasterDesk busca, o servidor nunca chama
de volta. O `SupportSystemProvider` não tem nenhum método de escrita; fechar
chamado, comentar e reatribuir continuam sendo feitos no Mastersys.

Não há webhook de tarefas/chamados no backend. Os únicos webhooks existentes são
do módulo WhatsApp, e são de **entrada**, recebendo do provedor ZPro. A rota
`tasks/notedesk/outbox` parece push à primeira vista, mas não é: o cliente faz
`GET` na fila e depois `POST` no ack — continua pull.

Mais importante: **um webhook pioraria o problema**. Ele inverte a direção — em
vez de o cliente conhecer o servidor, o servidor precisaria alcançar cada
desktop, o que exigiria tornar a máquina de cada usuário acessível de fora. O
que protege o backend é o reverse proxy, que já existe.

---

## 3. Onde ficam as credenciais

| O que | Onde |
|---|---|
| refresh token | cofre do SO (Windows Credential Manager) |
| access token | somente memória |
| endereço, id/nome/e-mail do usuário | `app_settings` no `masterdesk.db` |
| senha | **em lugar nenhum** — usada no login e descartada |

A senha trafega no `POST /api/auth/login`, protegida pelo TLS do domínio. O
esquema `http://` é aceito pela validação (é o que permite o teste local), então
**em produção confira que o endereço começa com `https://`** — nada no app
impede alguém de digitar `http://` e mandar a senha em claro.

---

## 4. Diagnóstico — se a sincronização falhar para algum usuário

1. **O proxy encaminha `/suporte/api` para o backend** com os métodos usados:
   `POST` no login/refresh e `GET` nas duas listagens. Um proxy que só libera o
   necessário para o frontend web pode estar restringindo verbos ou caminhos.
2. **`/api/tickets/paginated` responde** — é o endpoint real da sincronização
   (veja a ressalva na seção 5).
3. **Nada exige cabeçalho `Origin`/CORS**: as chamadas saem do Rust via
   `reqwest`, não do webview, então não passam por CORS. Um proxy que bloqueie
   requisições sem `Origin` conhecido quebraria o app.
4. **Janela de chamados**: o padrão é 90 dias
   (`DEFAULT_TICKET_WINDOW_DAYS`). Chamados mais antigos não sincronizam. Se a
   equipe trabalha com chamados longos, ajuste em **Tarefas → Mastersys**.

Teste sugerido: um usuário-piloto configura o endereço, sincroniza e confere se
a contagem de itens bate com o que ele vê no Mastersys.

---

## 5. Ressalva na documentação do MasterDesk

O guia `docs/INTEGRACAO_MASTERSYS.md` do MasterDesk documenta:

```
GET /api/tickets?assignedTo=:id
```

Mas o código (`crates/infrastructure/src/mastersys_provider.rs`, `fetch_tickets`)
usa:

```
GET /api/tickets/paginated?assignedTo=<id>&createdAtStart=<data>&page=<n>&pageSize=<n>
```

O próprio arquivo-fonte anota o motivo: `GET /api/tickets` (`TicketRepository.findAll`)
não tem filtro de `assignedTo`. Ao configurar proxy ou firewall, **libere o
caminho `/paginated`** — seguir o guia levaria a liberar a rota errada.

---

## 6. Sentido do fluxo — o que não dá para fazer

| Campo | Dono | No sync |
|---|---|---|
| título, descrição, prioridade, prazo, concluída | Mastersys | **sobrescritos** |
| anotações da tarefa | usuário | **nunca tocadas** |
| lembretes | usuário | definidos só na importação |

Editar um item espelhado no MasterDesk não tem efeito duradouro: a próxima
sincronização sobrescreve. Item que sai da fila do usuário (reatribuído,
cancelado) é removido do quadro — **exceto** se tiver anotações, caso em que
permanece marcado como concluído.
