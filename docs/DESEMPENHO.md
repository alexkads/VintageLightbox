# Desempenho — a ferramenta de medição do rodapé

> Pedido do dono (27/09/2026): investigar a renderização lenta, especialmente no Windows —
> quantos quadros a janela produz, quais engasgam, em qual etapa da CPU ou da GPU o tempo vai,
> e em que máquina (hardware, sistema, drivers), para separar defeito do app de limite do
> hardware.

O código está em `crates/ui-gpui/src/desempenho/` (interface), `crates/revelacao-core/src/cronometro.rs`
(tempo de GPU) e `crates/infrastructure/src/database/desempenho.rs` + migração `025_desempenho.sql`
(banco).

## Como usar

1. **Rodapé → Desempenho.** Abre uma **janela própria**, que vai para qualquer monitor e não
   rouba o foco da janela principal.
2. **Iniciar**, repetir a ação lenta (rolar a galeria ou a tira, trocar de foto, abrir a
   Revelação, arrastar um slider, pintar, gradiente, clone/heal), **Parar**.
3. **Salvar no banco** grava a sessão, em lote e em segundo plano, no SQLite do catálogo **e no
   servidor** (`POST /api/v2/app-desktop/desempenho`, tabela `app_desktop_desempenho` do Postgres
   de produção — desde a 0.1.30, pedido do dono: *"O ideal era salvar no banco de dados"*). Sem
   rede ou sem conta, ela entra numa fila em disco (`~/.vintagelightbox/desempenho/para-enviar/`)
   e sobe quando a janela de Desempenho abrir de novo; a mesma sincronização manda as sessões
   locais gravadas antes da 0.1.30.
   **Copiar relatório** leva o texto completo, com o diagnóstico e a máquina, para colar numa
   conversa.
4. **Sessões** lista as guardadas **no servidor, de todos os computadores** (mais as locais que
   ainda não subiram); **Comparar** põe a mesma operação lado a lado por sistema. Sem servidor, as
   duas mostram só este computador e dizem isso. Apagar do servidor é de quem tem
   `ViewSystemDiagnostics`; o operador apaga só a cópia local.
5. Enquanto grava, o botão do rodapé fica vermelho ("● Medindo"), mesmo com a janela fechada.

**Meça em build otimizado** (`--release`, ou o perfil `carga`). Em `debug` tudo custa até 57×
mais, e o relatório avisa.

## O que cada número é

| Número | Como é medido |
|---|---|
| **FPS durante a interação** | intervalos entre quadros **concluídos pela janela** durante gestos contínuos (rolar, arrastar, pintar). Não conta chamadas de `render`. |
| **Mediana / p95 / pior** | desses intervalos. |
| **Acima do orçamento** | intervalo > 1,5 × período do monitor **e** o quadro atrasado em relação ao gesto: respondeu em mais de dois períodos, ou a thread da interface estava presa (sem bater por mais de 48 ms) antes de processar o gesto. Com a entrada a 60 Hz num monitor de 120 Hz, a janela desenha a 60 Hz: isso é o ritmo do gesto, não engasgo. |
| **Tempo observado do quadro** | `render` da raiz → depois do `present` (aproximado). Mostrado **separado** das etapas: CPU e GPU trabalham em paralelo e não se somam. |
| **Montagem da interface** | `render` da raiz → fim da pintura (render + layout + pintura do GPUI). |
| **Apresentação (aprox.)** | fim da pintura → primeira tarefa que a thread roda depois do quadro. É um teto, não o valor exato do `present`. |
| **Etapas na thread da interface** | decodificação síncrona, preparação dos ajustes, recorte/giro, histograma, conversão RGBA→BGRA (`para_gpui`). Prendem o quadro. |
| **Etapas em segundo plano** | decodificação antecipada, redução, preparo no motor (upload, grades), gravação e envio dos comandos, espera pela GPU, cópia de volta. Atrasam a foto, não o quadro. |
| **GPU** | máscaras, retoques e revelação, por **timestamp query** nas passadas do motor. |
| **Do pedido à foto na tela** | ponta a ponta: do pedido ao motor até o resultado entrar no palco. |
| **Do gesto processado ao quadro seguinte** | latência de resposta a cada gesto. |

A **operação** de cada quadro vem de ganchos reais: rolagem (ouvinte de rolagem na fase de
captura, qualquer tela), troca de foto (`mostrar_a_posicao`), abertura da Revelação, slider
(`SliderEvent::Change`), pincel/gradiente/laço/clone/heal/preencher (gesto da Revelação local).

## O que não dá para medir (e o relatório diz)

- **Tempo de GPU do renderizador do GPUI** (DirectX 11 no Windows, Metal no macOS, wgpu no Linux)
  e a subida das imagens ao atlas dele: o GPUI 0.3 não expõe instrumentação. O `profiler` dele
  mede o `present`, mas liga contadores em todas as tarefas do app o tempo todo, e isso
  contaminaria a medição com a captura desligada.
- **O fim exato do `present`**: sem gancho público. A apresentação é medida até a primeira tarefa
  seguinte.
- **Tempo de GPU sem `TIMESTAMP_QUERY`**: sobram as etapas de CPU e a "espera pela GPU", que
  inclui fila e driver. Para simular essa GPU, rode com `VLB_SEM_CARIMBOS=1`.
- **Travas curtas antes do gesto**: a batida da thread é de 16 ms (o timer do Windows tem ~15,6 ms
  de grão), então só a ausência de três batidas prova a thread presa. Travas menores aparecem no
  tempo do quadro e nas etapas.
- **A segunda janela** (Tela do cliente, e a própria janela de Desempenho) não tem sentinela; o
  tempo que ela gasta é da mesma thread e aparece nos intervalos da principal.

## A máquina

Cada sessão leva CPU (modelo, núcleos, MHz), memória (total e livre), **todas** as GPUs que o
wgpu enxerga (fabricante, ids PCI, tipo, backend, driver, timestamps), a GPU que o motor abriu, a
placa como o sistema a descreve (no Windows, a versão e a data do driver pelo
`Win32_VideoController`), os monitores com resolução e Hz, a energia (bateria/tomada, plano do
Windows) e o sistema (versão, kernel, área de trabalho no Linux). Nada de hostname, número de série
ou usuário.

O diagnóstico usa isso: motor no OpenGL, num adaptador de software ("Microsoft Basic Render
Driver") ou na integrada com uma dedicada disponível é **defeito do app**, e não limite da máquina.

## Travamentos

Com a captura ligada, uma thread vigia percebe a interface parada por mais de 1 s e grava **na
hora** a sessão até ali em `~/.vintagelightbox/desempenho/pendentes/<id>.json`. O botão da
interface não conseguiria fazer isso: com ela travada, nada dela roda. Na próxima vez que a
janela de Desempenho abrir, esses arquivos entram no banco com a origem `travamento`.

## Memória

Anéis de tamanho fixo (36 mil quadros, 3 mil lentos e os 50 piores, 5 mil revelações) e
distribuições por operação × etapa de tamanho fixo. A captura fica abaixo de 8 MB, qualquer que
seja a duração (teste `a_captura_enorme_respeita_o_teto_de_memoria`, com 400 mil quadros). Ao
salvar vão todos os lentos, uma amostra uniforme de até 2 mil quadros normais e até 300
revelações.

## Cenário reproduzível (Windows × Linux × macOS)

O roteiro de depuração roda num binário otimizado com a feature `roteiro`; o do balcão não a
tem:

```bash
cargo build --profile carga -p ui-gpui --features roteiro
```

Os passos `varrer` (arrasto), `rolar` e `desempenho iniciar|parar|salvar|relatorio|foto <nome>`
entram pelo `dispatch_event` do GPUI e valem nos três sistemas. Use a **mesma foto** (restaure o
catálogo antes de cada rodada: a revelação grava no catálogo), o mesmo tamanho de janela e a
mesma sequência. Exemplo:

```text
tamanho 1600 1000
revelar
desempenho abrir
desempenho iniciar
varrer 120 8 1427 334 1540 334
rolar 40 16 800 744 -40
tecla right
tecla k
varrer 120 8 600 250 1000 450
desempenho parar
esperar 5000
desempenho relatorio
desempenho salvar
fim
```

Para comparar máquinas basta salvar em cada uma: a aba **Comparar** lê o servidor. **Exportar** e
**Importar** continuam para quem mede sem conta (ou para levar a sessão a outro lugar).

## Comparar as APIs gráficas

O botão **Comparar APIs gráficas** da janela Desempenho (pedido do dono em 9/out/2026, 0.1.128)
revela a mesma foto sintética de 6000×4000 em **cada API da máquina** (DX12, Vulkan e OpenGL no
Windows; Vulkan e OpenGL no Linux; Metal no Mac), em toda placa que cada uma enxerga, nos três
tamanhos que a Revelação pede (rascunho do arrasto, 2560 ao soltar, a foto inteira). No Windows mede
também a **subida ao DirectX 11** da janela, do jeito que o atlas do GPUI faz, e soma: motor + BGRA
+ subida = um quadro do slider. A captura do Iniciar/Parar mede só a API em que o motor abriu.

- Roda num **processo filho** do app (`--comparar-apis <arquivo>`): abrir uma API que o app não usa
  carrega o driver dela, e um driver ruim derruba quem o carregou. O filho grava o resultado a cada
  passo; se cair, o app salva o que veio e diz em que passo foi.
- A sessão vai ao banco sozinha (computador e servidor), com `origem = comparar_apis`, uma métrica
  por API × placa × tamanho (`revelacao_<tamanho>`) e por subida (`subida_dx11_<tamanho>`); o
  resultado inteiro fica em `revelacoes_json`, e a aba Sessões refaz o relatório dele.
- O código: `crates/revelacao-core/src/comparacao_de_apis.rs` (a medição) e
  `crates/ui-gpui/src/desempenho/comparacao.rs` (o filho e a sessão).

Para uma máquina sem o app, o mesmo em binário, que grava `medir-gpu-<data>.txt` ao lado:

```bash
cargo run --release -p revelacao-core --bin medir-gpu -- foto.jpg
# o .exe para Windows, feito no Mac (o GPUI não entra, então o fxc não é preciso):
cargo build --profile carga --target x86_64-pc-windows-gnu -p revelacao-core --bin medir-gpu
```

No Mac (M2 Pro, 9/out/2026): o quadro do rascunho em 3,6 ms, a foto inteira em 8,6 ms (`--release`).
