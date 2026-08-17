# O objetivo do VintageLightbox

**Escrito em**: 17 de agosto de 2026 — o dia em que o objetivo mudou.

> **Substituir o Lightroom no fluxo do estúdio — para que a edição converse com o
> `recordarfotos.com.br`.**
>
> Como se chega lá: um editor de fotos completo e funcional, no formato do Lightroom, em Rust com
> GPUI. **Completo**: importar, organizar, triar, revelar e **entregar arquivo**. **Funcional**:
> todo controle que a tela oferece move a foto — um slider que existe e não faz nada é defeito, não
> pendência.

## 🔑 Por que clonar o Lightroom, e não usar o Lightroom

Esta é a pergunta que o projeto inteiro responde, e ela não estava escrita em lugar nenhum até
17/ago/2026.

O fluxo do estúdio hoje **passa pelo Lightroom**, e o Lightroom **não conversa com o
`recordarfotos.com.br`**. O site é onde o cliente:

1. **baixa o ensaio que já comprou** — as fotos adquiridas no estúdio;
2. **compra as que ficaram para trás** — as que ele viu, não levou, e pode querer depois.

Entre a revelação e essas duas coisas existe hoje um vão que se atravessa **na mão**: exportar,
separar o que foi comprado do que não foi, subir, montar a galeria do cliente. Cada passo manual é um
lugar onde a foto errada vai para a galeria errada.

**A ferramenta existe para fechar esse vão.** Não é "um Lightroom melhor" — é o Lightroom **no lugar
certo do ecossistema**, onde a decisão que o fotógrafo já toma (esta foi comprada, esta não) vira
diretamente o que o cliente vê no site.

⚠️ **Isso reordena o que importa.** Duas funcionalidades do Lightroom são infraestrutura deste
objetivo, e não itens de lista:

| | Por quê |
|---|---|
| **Exportação** | é o que alimenta a galeria. Sem ela não há nada para o site receber |
| **Coleções** | "o ensaio do cliente" **é** uma coleção; "comprada" e "deixada para trás" são a divisão dentro dela |

E duas que parecem centrais no Lightroom **não são** aqui: os módulos Livro, Slideshow, Mapa e Web, e
os ajustes locais com máscara. Um estúdio de retrato entrega revelação global; pincel é o que se usa
em uma foto de cem.

## O que vem depois — e só depois

A integração com a API de pós-venda — e **"depois" é literal**: ela começa quando o clone estiver
funcional, e não em paralelo (decisão do dono, 17/ago).

O app tem de ser útil sozinho antes de conversar com qualquer coisa. Um fluxo que já depende do site
para funcionar não tem como ser adotado aos poucos, e adoção aos poucos é a única que dá para
desfazer. **Enquanto houver item na [fila](PARIDADE-LIGHTROOM.md), a fila é o trabalho.**

---

## 🚨 O que este documento substitui, e por quê

Até 17/ago/2026 o objetivo canônico era **outro**, e está registrado em
[`10-MIGRACAO-GPUI.md`](historico/10-MIGRACAO-GPUI.md): trocar a interface de egui por GPUI **com paridade**,
sem regressão. Esse objetivo foi **alcançado** — as cinco fases fecharam, o `crates/ui` saiu do
workspace (`8c7df32`, −25.783 linhas) e o `Cargo.lock` não tem um pacote `egui` sequer.

O problema é que as regras daquele objetivo **continuavam valendo, e agora atrapalham**. Elas diziam,
textualmente (§7 de lá):

| Regra da migração | Por que existia | Por que morre aqui |
|---|---|---|
| ❌ **Nenhuma feature nova** | feature nova torna impossível saber se uma diferença é defeito de porte ou escopo que só um lado tem | não há mais dois lados para comparar |
| ✅ O `crates/ui` compila até a fase 5 | era o rollback e a referência | ele não existe |
| ❌ Nunca traduzir componente egui linha a linha | modo imediato traduzido para retido vira o pior dos dois | não há mais componente egui |

🔑 **A regra "nenhuma feature nova" é a que mais custou.** Ela é o motivo de o painel de Revelação ter
**19 sliders que não fazem nada**: o app de egui também não os aplicava, então implementá-los seria
feature nova, e o porte ficou fiel ao defeito. Fidelidade era a coisa certa **enquanto o alvo era o
app antigo**. O alvo agora é o Lightroom, e a mesma decisão passa a ser o contrário de certa.

⚠️ **O que **não** muda com o objetivo novo**: `10-MIGRACAO-GPUI.md` continua valendo como
**história**, e é a melhor fonte que existe sobre por que o código é como é — as armadilhas do GPUI,
o BGRA das texturas, o `uniform` que casa por posição, o foco que não se concede. Ele deixou de ser
plano; não deixou de ser verdade.

---

## Teste de alinhamento — antes de começar qualquer trabalho

1. **Um fotógrafo consegue fazer isto no Lightroom?** Se não, é escopo além do alvo — pergunte antes.
2. **A tela promete isto e não entrega?** Então é **defeito**, e defeito tem prioridade sobre
   funcionalidade nova. Controle que responde sem mover a foto é a pior categoria: ele não falha, e
   quem usa culpa o próprio olho.
3. **Isto pode ser conferido sem abrir o app?** Se não dá para escrever um teste que falhe hoje e
   passe depois, o trabalho ainda não está entendido o bastante para começar.
4. **Isto é a coisa mais barata que destrava mais coisa?** Exportação destrava o app inteiro; um
   décimo modo de organizar coleção não destrava nada.

**Fora do alvo, e é decisão, não esquecimento**: módulos Mapa, Livro, Slideshow e Web do Lightroom;
sincronização com nuvem; catálogo compartilhado por rede. O produto é **um** fotógrafo, **uma**
máquina, **um** catálogo.

---

## Os critérios de "funcional"

Não é opinião, e por isso cada um tem como ser medido:

| Critério | Como se confere |
|---|---|
| **Todo controle da tela move a foto** | ✅ **os 42 passam desde 17/ago** — um teste por família, medindo pixel |
| **O arquivo exportado é o que a tela mostra** | ✅ já vale: `crates/infrastructure/tests/exportacao.rs` |
| **RAW de câmera abre** | um arquivo por fabricante no acervo de teste |
| **Nada trava a janela** | rolagem a 60fps em `--release`, `medir-miniaturas` e `medir-abertura` |
| **Nenhum botão anuncia o que não faz** | `grep -ri "coming soon\|em breve\|TODO" crates/ui-gpui/src` volta vazio |

⚠️ **O último é o mais fácil de burlar e o mais importante.** Um botão que abre um aviso de "em
breve" é pior que um botão ausente: ele ocupa o lugar da funcionalidade e some do inventário mental
de quem lê a tela.

---

## Onde continuar

1. **[PARIDADE-LIGHTROOM.md](PARIDADE-LIGHTROOM.md)** — a lista medida do que funciona, do que a tela
   promete e não faz, e do que não existe. **Comece por aqui**: é a fila de trabalho.
2. **[STATUS.md](STATUS.md)** — o estado do código, camada por camada, e as lacunas conhecidas.
3. **[10-MIGRACAO-GPUI.md](historico/10-MIGRACAO-GPUI.md)** — a história, e o melhor registro das armadilhas do
   GPUI que já custaram commit.

## Convenções que continuam valendo

Vieram da migração e não dependiam dela:

- **Um incremento por commit**, com mensagem em português contando o que foi **encontrado**, e não
  só o que foi feito.
- **Teste conferido quebrando de propósito.** Um teste que passa com o código quebrado já custou dois
  commits inteiros afirmando um atalho que nunca respondeu.
- **Toda medida de desempenho em `--release`.** Em `debug` uma miniatura custa 56× mais, e o
  framework quase foi condenado por medida no perfil errado.
- **Defeito preservado de propósito fica registrado em teste**, com a razão escrita ao lado.
