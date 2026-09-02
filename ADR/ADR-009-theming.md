# ADR-009 — Tema claro / escuro / automático

**Status:** Aceito (2026-09-02)

## Contexto

A seção 9 do CLAUDE.md coloca customização visual como requisito central, com
tema no topo da lista. O pedido concreto: claro, escuro e "automático do
sistema".

Duas dificuldades específicas deste app, que um tema escuro comum não tem:

1. **A detecção do tema do SO não é confiável pelo caminho óbvio.** A
   propagação do tema da janela do Tauri para o `prefers-color-scheme` do
   webview é inconsistente entre plataformas, e é bug aberto no Linux
   (tauri-apps/tauri#9255, tauri-apps/wry#806). Confiar só na media query
   faria o modo automático simplesmente não funcionar em parte dos SOs que o
   projeto declara suportar.

2. **O usuário escolhe a cor de cada nota.** Um post-it amarelo `#FFEB3B` renderizado
   cru sobre fundo escuro é uma lanterna. E as cores não são um conjunto
   fechado: existe `<input type="color">`, então qualquer hex é possível.

## Opções

### Detecção do tema do sistema

- **(1a) Só `prefers-color-scheme`.** Simples, mas quebra no Linux pelo bug acima.
- **(1b) Só `getCurrentWindow().theme()` do Tauri.** Consulta o SO pelo lado
  Rust, mas não funciona ao rodar `npm run dev` no navegador, que é o loop de
  desenvolvimento do frontend.
- **(1c) Os dois canais, com a API nativa como fonte da verdade** e a media
  query como fallback e como leitura síncrona do primeiro paint.

### Persistência da escolha

- **(2a) `app_settings` no SQLite.** Consistente com o resto, mas assíncrono:
  causaria um flash de tema claro a cada abertura, inclusive em cada janela de
  nota destacada.
- **(2b) `localStorage`.** Leitura síncrona antes do React montar. As janelas de
  nota compartilham a origem, então compartilham o valor.

### Cores das notas no tema escuro

- **(3a) Manter a cor exata.** Fiel ao Sticky Notes, mas gera ilhas de luz.
- **(3b) Cor apenas na borda/cabeçalho.** Sóbrio, mas perde o visual de post-it,
  que é a identidade do produto.
- **(3c) Tone-mapping automático:** preservar o matiz, recalcular saturação e
  luminosidade.

## Decisão

**(1c) + (2b) + (3c).** Escolha de (3c) feita pelo DEV em 2026-09-02.

### Tokens divididos por papel

O CSS anterior usava `--ink` como cor de texto **e** como fundo da barra de
navegação e dos botões sólidos (`background: var(--ink); color: #fff`). Sob
inversão isso produziria nav branca e botão branco-no-branco. Então os tokens
passaram a ser semânticos: `--text*`, `--canvas`/`--surface*`, `--line*`,
`--chrome*`, `--action*`, `--accent*`.

Duas consequências deliberadas dessa divisão:

- **A nav fica escura nos dois temas.** É a faixa que dá identidade ao app; no
  claro ela é o contraste que faz o MasterDesk não parecer um app branco
  genérico. No escuro ela vai um degrau **mais escura** que o canvas, para
  continuar lendo como moldura e não como painel flutuante.
- **`--action` troca de valor, não só de tom.** No claro, botão sólido é
  quase-preto com texto branco. No escuro, quase-preto sobre fundo escuro
  desapareceria, então o papel de "ação" passa para o acento amarelo com texto
  escuro. Nenhum componente sabe disso — todos usam `var(--action)`.

Nenhum componente tem regra própria de tema. Só os tokens mudam, o que impede o
par claro/escuro de divergir conforme a UI cresce.

### Tone-mapping das notas (`frontend/src/theme/noteSurface.ts`)

- Matiz preservado: é ele que identifica a nota de relance.
- No escuro: saturação limitada a 0,18–0,42 e luminosidade fixada em ~16%, um
  degrau acima do canvas. Amarelo vira âmbar profundo, azul vira azul-noite.
- Cor acromática (branco, preto, cinza) cai num grafite morno — "branco" no
  tema escuro não pode ser branco.
- A cor do texto **não** vem de uma tabela: é escolhida por razão de contraste
  WCAG 2.1 real contra o fundo resultante.

### Piso de contraste

Isto corrigiu um defeito que já existia. A função anterior, `textColorFor`, era
uma tabela com as 8 cores predefinidas e devolvia texto escuro para qualquer
outra — então uma cor escura escolhida no seletor personalizado ficava ilegível.

Ao medir o contraste real apareceu um segundo problema: o rosa `#E91E63` do
próprio preset não atinge AA (4,5:1) com **nenhuma** cor de texto — fica em
~4,23:1. Escolher "o menos pior" deixa o texto naquela faixa quase-legível que
passa despercebida em revisão e cansa quem usa o dia inteiro.

Decisão: quando nenhuma cor de texto atinge AA, **o fundo cede**. A luminosidade
caminha em passos de 2% e vence o primeiro valor que atinge 4,5:1 — o menor
desvio possível. Matiz e saturação ficam intactos, então a nota continua "a
rosa" (`#E91E63` → `#EB3170`, 4,56:1).

Garantido por teste: 36 combinações cor × tema, todas ≥ 4,5:1
(`frontend/src/theme/noteSurface.test.ts`).

## Consequências

### Positivas

- O modo automático funciona mesmo onde o `prefers-color-scheme` do webview
  falha, porque a API nativa é a fonte da verdade.
- Sem flash de tema: `applyInitialTheme()` roda antes do React, e o
  `index.html` já pinta o fundo por media query antes de o CSS carregar.
- Trocar o tema em qualquer janela propaga para as notas destacadas por evento
  do Tauri (`masterdesk://theme-changed`).
- Toda cor de nota — inclusive personalizada — passa AA nos dois temas, o que
  antes não era verdade nem no tema claro.
- `color-scheme` acompanha o tema, então scrollbar, seletor de data e
  `<input type="color">` são desenhados pelo SO na variante certa.

### Negativas e limitações conhecidas

- A preferência mora no `localStorage`, fora do banco: não vai junto num backup
  do `masterdesk.db` e não é sincronizável. Aceitável por ser preferência de
  máquina; se um dia precisar viajar com o usuário, migra para `app_settings`.
- Em janela privada ou com storage bloqueado a escolha não persiste; a sessão
  atual funciona e o modo volta a "sistema" na reabertura.
- **Validado apenas em Windows 11.** `onThemeChanged` e `theme()` precisam de
  verificação manual em macOS e Linux (X11 e Wayland) antes de declarar o modo
  automático suportado neles — o bug citado é justamente de Linux.
- `color-mix()` é usado em três lugares com fallback opaco declarado antes.
  Onde não houver suporte, o header sticky fica opaco em vez de translúcido.

## Dependência adicionada

| | vitest |
|---|---|
| Versão | 3.2 |
| Propósito | testes unitários do frontend (tone-mapping e resolução de tema) |
| Documentação | https://vitest.dev |
| Licença | MIT |
| Escopo | `devDependencies` — não entra no bundle |
| Por que | é o runner nativo do Vite, que o projeto já usa (ADR-002); reaproveita o mesmo `tsconfig` e resolução de módulos, sem configuração adicional |
| Alternativas | Jest (exige configuração de transform própria para TS/ESM); nenhum teste (rejeitado: o piso de contraste é uma invariante que precisa de proteção contra regressão) |
