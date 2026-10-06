# 10 — Etapa 7: a imagem editada vai ao site (D23)

> Pedido do dono (06/out/2026): *"continue"*, depois da etapa 6. A divergência D23 do `CONTRATO_DA_FOTO` era
> um defeito de verdade: a revelada que subia já tinha o retoque, mas **qualquer revelação feita depois no
> site partia do bruto** — a do funcionário no painel e a do cliente em `/meus-ensaios` — e o retoque sumia.

## O desenho

| Ponta | O que mudou |
|---|---|
| Banco | `pos_venda_fotos.caminho_editada` e `editada_revisao` (migration `20261006120000_editada_do_balcao`) |
| Domínio do site | `PosVendaFoto::fonte_para_revelar()` = a editada quando há, senão o bruto. **O `fonte_da_revelacao()` (e o `/original`) continua sendo o bruto**: é a base do editor em camadas daqui, que confere a impressão dela ao reabrir o projeto, e o que volta ao catálogo local (C21) |
| API | `POST /api/v2/public/pos-venda/editada/{bilhete}` (o bilhete de revelação da foto; `file` JPEG + `revisao`), `DELETE /api/v2/pos-venda/fotos/{id}/editada`, `GET /api/v2/pos-venda/fotos/{id}/fonte-para-revelar`; `editada_revisao` na foto |
| Quem parte da editada | o editor de revelação do painel (rota nova `fonte-para-revelar`), a cópia de trabalho (com a revisão no nome, para não servir a de antes), o editor do cliente em `/meus-ensaios`, o "Zerar tudo", as prévias e o download da foto neutra |
| Desktop | o "Salvar na galeria" de uma foto editada sobe a editada (PNG local → JPEG 95, com a revisão do projeto) antes da revelada; sem edição local, revela a partir da **fonte do site** (o retoque de outro balcão), e não do bruto; "Excluir a edição" marca a foto, e o envio seguinte tira a editada do site antes de revelar do bruto |

🔑 **Revisão monotônica**: o site ignora uma revisão que não é mais nova que a gravada — o envio atrasado de
um balcão não desfaz o retoque mais novo. **Só o gesto do próprio balcão tira a editada**: um balcão que não
tem o projeto nunca a apaga.

## Conferência

- Backend: os testes da aplicação (`a_editada_entra_pela_revisao_e_a_velha_nao_passa_por_cima`,
  `a_editada_tem_de_ser_jpeg`, `zerar_com_editada_parte_da_editada`,
  `a_fonte_para_revelar_e_a_editada_e_o_original_segue_o_bruto`) e o contrato no Postgres
  (`a_editada_do_balcao_vira_a_fonte_do_site_e_o_bruto_nao_muda`), com o módulo inteiro do pós-venda verde.
- Site: os 1148 testes do módulo de sessões e a checagem de tipos.
- Desktop: o cliente HTTP (`a_editada_sobe_sai_e_a_fonte_tem_rota_propria`), a conversão e a revisão, e o
  **e2e do ciclo de vida contra a API de verdade** (`o_retoque_do_balcao_vai_ao_site_e_sai_com_a_edicao`):
  editar a foto da sessão, salvar, "Salvar na galeria", a editada no site com a revisão 1, a fonte da
  revelação lá diferente do bruto e o `/original` igual a ele; excluir a edição, salvar de novo, e a fonte
  volta ao bruto.

## Limite declarado

A marca de "edição excluída" vive na memória do app: excluir a edição e fechar o app antes do "Salvar na
galeria" deixa a editada no site até a próxima vez que a foto for editada e salva.
