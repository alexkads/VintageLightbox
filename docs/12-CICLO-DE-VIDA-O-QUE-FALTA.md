# E2E do ciclo de vida — o que falta

**Escrito em**: 3 de outubro de 2026. **Situação**: plano aprovado, **não começado**. O dono
termina depois.

O e2e do ciclo de vida (`crates/ui-gpui/src/e2e/ciclo_de_vida/`, `make e2e-ciclo`) roda o app de
verdade contra uma API descartável, com o proxy de falhas no meio. Ele tem 23 cenários, e o
`make producao` não sobe nada sem eles verdes.

## O desequilíbrio

O caminho feliz passa pelas sete etapas uma vez. Os caminhos difíceis (triste, tortuoso e
tenebroso) ficaram concentrados no dinheiro:

| Etapa | Cenários além do feliz |
|---|---|
| Caixa e pagamento (venda, estorno, sangria, fechamento, PIX) | cerca de 15 |
| Subida das fotos / criação | 3 |
| **Classificação, sinalização e negociação** | **2** |
| Revelação | 1 |
| Pós-venda (link, compra, download) | 1 |
| 2ª tela do cliente | 0 |

## 🚨 Prioridade: classificação, sinalização e negociação

O dono, 03/out/2026: *"a parte da classificação, sinalização e negociação é muito importante, pois
se ele quebrar o usuário para imediatamente de usar e abre o Lightroom ou Darktable!"*

Por isso a régua aqui não é só "funciona". É **responder na hora e nunca perder uma marca**. Uma
tecla morta, uma marca que some ou uma tela lenta contam como quebra.

### Classificação e sinalização (nota, rejeição, levar com `B`)

1. **Rajada real.** 200 fotos marcadas no ritmo de um operador rápido (seta, nota, `B`,
   rejeitar). Nenhum gesto se perde e o site termina igual à tela. O tempo de resposta de cada
   tecla é medido, com teto.
2. **A rede cai no meio da rajada.** A tela não trava nem mente. As marcas sobem quando a rede
   volta, e a ordem é respeitada (vale a última).
3. **Mudar de ideia.** Nota 5, depois 3, depois 0; `B` duas vezes; rejeitar e desfazer. O final
   está certo no site.
4. **Tecla morta.** Abrir e fechar cada painel, diálogo, aviso e a 2ª tela, e conferir que as
   teclas continuam respondendo (ver a sonda de foco e o `Balcao::foco_vivo`).
5. **Filtros.** Por estrela e por levadas: as setas não pulam foto, e a lista se atualiza ao
   marcar.
6. **O app fecha no meio da rajada.** Ao reabrir, todas as marcas continuam lá.
7. **Duas pontas ao mesmo tempo.** O app e o painel do site marcando a mesma sessão terminam
   iguais.
8. **Sessão grande.** Mais de 500 fotos sem a tecla ficar lenta.

### Negociação (cortesia, desconto, faixa de preço)

1. **Por foto e em lote.** O cupom tem de bater o valor certo, sempre partindo do preço cheio
   (`preco_de_balcao`, nunca o `price` com desconto).
2. **A rede cai durante a negociação, e negociar e levar vão na mesma rajada.** Um não desfaz o
   outro.
3. **Valor inválido, zero ou com vírgula.** Tirar o desconto volta ao preço cheio.
4. **Foto já vendida ou em pagamento online.** A negociação é recusada com uma frase clara.

## Depois: as outras etapas sem cobertura

- **Seleção com o cliente.** A 2ª tela cai ou desconecta no meio; troca rápida de foto; duas
  máquinas na mesma sessão.
- **Revelação.** Predefinição, corte e ajustes salvos e reabertos iguais; revelar em lote; a
  revelação local sobe ao site; a foto comprada bloqueada para mexer; retomar de onde parou.
- **Criação.** Importação grande, arquivo repetido, cartão retirado no meio, sessão sem e-mail
  até o fim.
- **Pós-venda.** O e-mail de aviso sai; o cliente abre o link em outro aparelho; download do
  original; prazo de venda vencendo e prorrogação; sessão excluída e restaurada.
- **Sessão no dia seguinte.** Fechar o app, reabrir em outra máquina e continuar de onde parou.

## Como fazer (as regras que já valem para a suíte)

- **Tudo pela tela.** Use `clicar` por `debug_selector`, `teclar` e `digitar`; nunca chame um
  método por dentro. Cada ato fecha com `ato_limpo`: foco vivo e nenhum erro na tela.
- **Afirme três coisas.** O que a tela mostra, o estado por trás dela e o que a API gravou
  (pela `api_direta`).
- **Falhas pelo proxy.** Use `Falhas::programar` com `cair`, `status`, `atrasar` ou `engolir`.
- **Quebre de propósito.** Cada cenário novo tem de falhar com o defeito que ele protege (seção 1
  do `07-E2E-TESTING.md`).
- **Leia o log inteiro da rodada verde.** O aviso de foco perdido já passou verde uma vez.
- **Corrija até ficar verde.** Um defeito que o cenário achar é corrigido no produto; não se
  afrouxa o teste.
- **Cuide da máquina.** Um build do app por vez, `CARGO_INCREMENTAL=0`, e confira o `df` antes.
  O servidor do ciclo tem token de 15 minutos: se ficar de pé mais que isso, reinicie.
