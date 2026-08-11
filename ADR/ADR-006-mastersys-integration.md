# ADR-006 — Integração Mastersys

**Status:** Bloqueado — depende de informação externa

## Por que este ADR está vazio por enquanto

A seção 10 do CLAUDE.md é explícita: "Never implement a Mastersys API call until
the real API contract has been validated. If the API documentation is unavailable:
Ask the DEV." Nenhuma pesquisa pública substitui a documentação real da API do
Mastersys — não existe fonte confiável para "adivinhar" esse contrato.

## O que é necessário do DEV antes de preencher este ADR

```text
O que é conhecido: Mastersys é o sistema de suporte a integrar; MasterDesk deve
  se comunicar com ele via SupportSystemProvider/MastersysProvider.
O que é desconhecido: endpoints reais, formato de autenticação, formato de
  ticket/task, rate limits, versionamento da API.
Por que importa: qualquer suposição aqui vira dívida técnica ou bug de produção
  quando a API real divergir.
Opções: (a) aguardar documentação oficial do Mastersys; (b) aguardar acesso a um
  ambiente de sandbox/staging do Mastersys para engenharia reversa assistida
  pelo DEV.
Recomendação: aguardar documentação oficial antes de qualquer código de adapter.
Decisão necessária: o DEV deve fornecer a documentação da API ou definir como o
  time terá acesso a ela.
```

## Ação

Este ADR só pode ser escrito depois que o DEV fornecer a documentação/contrato real
da API do Mastersys. Até lá, `SupportSystemProvider` permanece como interface
vazia/abstrata no domínio, sem implementação concreta.
