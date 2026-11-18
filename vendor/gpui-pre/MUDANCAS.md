# gpui-pre 0.3.7 — controle local (#35)

Snapshot de crates.io, Apache-2.0 (LICENSE-APACHE). Mesmo snapshot do backend
Linux já vendorizado: zed@1a28cff.

- `window.rs`: habilitação explícita da coleta de acessibilidade; inserção
  literal de texto pelo input handler; consulta do foco de controles compostos;
  despacho de ações somente em nós atuais que anunciam e implementam a ação.
- `window/a11y.rs`: mantém a árvore ativa quando o controle local está ligado.
- `window/a11y/debug.rs`: consulta de ações implementadas, bounds e estado disabled
  no JSON existente. IDs reais já eram expostos como strings decimais.
- `platform.rs` e `window.rs`: tornam o contrato de captura da cena disponível
  fora de test-support. Implementações continuam específicas da plataforma.

Ao atualizar GPUI, reaplicar esses pontos e conferir os e2e de controle e a
janela real. A árvore só tem custo adicional com VLB_CONTROLE habilitado.
