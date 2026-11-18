# Controle do VintageLightbox por CLI e SSH (#35)

O canal local permite consultar a interface e enviar os mesmos eventos e ações
usados pelo operador. É desativado por padrão. A CLI `vlb` conversa com a
instância aberta, preservando autenticação, catálogo e regras do app.

## Compilar e habilitar

```sh
cargo build --release -p ui-gpui -p vlb-controle --bin ui-gpui --bin vlb
# Mac sem Xcode completo: acrescentar --features ui-gpui/shaders-em-tempo-de-execucao

# Na sessão gráfica onde a janela deve abrir:
export VLB_CONTROLE="$HOME/.local/state/vintagelightbox/controle"
./target/release/ui-gpui
```

Use uma pasta exclusiva para o controle. No Unix ela fica com permissão 0700,
e o arquivo de descoberta, 0600. Uma trava impede duas instâncias de usar a
mesma pasta. Sem `VLB_CONTROLE` não há listener nem coleta extra da árvore.
Para desabilitar, encerre o app e abra novamente sem a variável.

O instalador também entrega a CLI: `~/.local/bin/vlb` no Linux,
`VintageLightbox (Zed GPUI).app/Contents/MacOS/vlb` no macOS e `vlb.exe` na
mesma pasta do executável instalado no Windows
(`%LOCALAPPDATA%\Programs\VintageLightbox-GPUI`).

No PowerShell:

```powershell
$env:VLB_CONTROLE = "$env:LOCALAPPDATA\VintageLightbox\controle"
$app = "$env:LOCALAPPDATA\Programs\VintageLightbox-GPUI"
& "$app\VintageLightbox-GPUI.exe"
# Em outro terminal, na mesma máquina/conta:
& "$env:LOCALAPPDATA\Programs\VintageLightbox-GPUI\vlb.exe" --dir "$env:LOCALAPPDATA\VintageLightbox\controle" status
```

Se compilou manualmente, use `target\release\ui-gpui.exe` e
`target\release\vlb.exe` em vez dos caminhos da instalação.

No macOS/Linux o executável também aceita `ui-gpui --controle --dir PASTA status`,
antes de abrir catálogo/chaveiro/janela. No Windows prefira `vlb.exe`, que
preserva stdout e código de saída no console (o app gráfico usa outro subsistema).

## Comandos

As opções globais vêm antes do comando. Saída: uma linha JSON
`{"ok":true,"result":...}` ou `{"ok":false,"error":"..."}`.
Código de saída: 0 sucesso, 1 erro de transporte/app, 2 argumento inválido.

```sh
export VLB_CONTROLE="$HOME/.local/state/vintagelightbox/controle"
vlb status
vlb inspect                         # controles, IDs, foco e ações
vlb inspect 'e-mail'                # filtro sem diferenciar maiúsculas
vlb tree                            # árvore completa para diagnóstico
vlb node 123456789 Focus            # ID decimal retornado por inspect
vlb text 'pessoa@example.test'
vlb key tab
vlb key ctrl-a
vlb click 320 180
vlb click 320 180 right
vlb scroll 980 450 -300              # negativo = para baixo
vlb drag 600 300 740 300
vlb node 123456789 Click
vlb node 123456789 SetValue 'texto'
vlb node 123456789 SetNumericValue 0.75
vlb screenshot /tmp/vintage.png
vlb --window 4294967297 inspect
vlb --timeout 30000 screenshot /tmp/vintage.png
vlb action 'nome::DaAcao'
vlb action 'nome::DaAcaoComDados' '{"campo":"valor"}'
vlb request '{"op":"describe","query":"Button"}'
```

`status` enumera janelas, PID e versão. Sem `--window`, os comandos usam a
janela principal. `inspect` devolve `controls`, `focus`, `frame` e `actions`.
As ações são as disponíveis no foco atual. Os IDs são strings decimais para
preservar 64 bits: use `controls[].id`, não as letras da árvore de diagnóstico.
Consulte novamente após navegar; controles removidos e ações sem implementação
retornam erro. `bounds` e comandos de mouse usam pontos relativos ao conteúdo
da janela. `tree` expõe `bounds_physical` em pixels.

Controles ainda sem descrição de acessibilidade não aparecem na árvore. Para
eles, use ações, teclas ou coordenadas e confira o visual com uma captura.
Inserção de texto exige um campo com foco, preserva Unicode e não muda o clipboard.

Importação, exportação e navegação podem usar as ações/controles expostos pela
interface e os comandos de entrada para preencher seus formulários. Continuam
passando por autenticação, permissões, catálogo e sincronização. Esta entrega
não cria uma API de negócio paralela nem execução arbitrária de shell.

Sucesso de uma mutação confirma o despacho. Rede, diálogos e processamento
podem terminar depois: inspecione novamente para confirmar. Timeout não prova
que o gesto falhou; consulte antes de repetir operações que gravam dados.
Pedidos que expiram antes do despacho são descartados. Timeout: 1 a 60000 ms,
padrão 10000 ms.

## Por SSH

O listener liga apenas em `127.0.0.1`, numa porta efêmera. Endereço e token ficam
no `controle.json` da pasta privada. Use a mesma conta do processo gráfico via
SSH: não é necessário expor portas ou usar RDP. A janela já deve estar aberta
numa sessão gráfica; a CLI não precisa de DISPLAY.

```sh
ssh alexdasilva@100.110.180.47 \
  '/caminho/vlb --dir "$HOME/.local/state/vintagelightbox/controle" inspect'
ssh alexdasilva@100.110.180.47 \
  '/caminho/vlb --dir "$HOME/.local/state/vintagelightbox/controle" screenshot /tmp/vintage.png'
scp alexdasilva@100.110.180.47:/tmp/vintage.png ./vintage.png
```

A captura é gravada na máquina do app. A CLI torna o caminho absoluto na máquina
onde ela roda (normalmente a mesma, via SSH).

## Captura por plataforma

- macOS: reutiliza CoreGraphics para capturar a própria janela.
- Linux X11/Wayland: rasteriza a cena no renderer WGPU com o atlas da janela
  e lê a textura para PNG, sem fotografar o desktop.
- Windows: habilita a captura da cena DirectX existente no GPUI fora dos testes.

No macOS a janela precisa estar visível. Diálogos do sistema e janelas de outros
processos não fazem parte da cena capturada/controlada.
O PNG do macOS inclui a barra de título; as coordenadas dos comandos continuam
relativas ao conteúdo abaixo dela. Prefira `bounds` da inspeção para clicar.

## Verificação

```sh
cargo test -p vlb-controle
cargo test -p ui-gpui --features shaders-em-tempo-de-execucao --lib e2e::controle
cargo clippy -p vlb-controle -p ui-gpui --features shaders-em-tempo-de-execucao -- -D warnings
```

O transporte testa Unicode, autenticação, pedidos inválidos/grandes demais,
timeout, descoberta local, permissões, trava e limpeza. Os e2e passam pelo
dispatcher de produção com os serviços de memória dos demais cenários; não
acessam a API de produção.

Conferido na janela real do macOS em 17/11/2026, em build release com catálogo
isolado: 40 consultas consecutivas, inspeção com filtro, foco, texto Unicode,
SetValue, ação do app, clique por ID e por coordenada, rolagem, arrasto e captura
PNG. Os testes de protocolo e dois cenários de UI passaram. `cargo check -p
ui-gpui` também passou no Fedora 44 por SSH.

Conferido na janela real do Fedora 44, GNOME/Wayland, em 100.110.180.47:
o mesmo roteiro de controle passou, incluindo 40 consultas consecutivas,
inspeção, foco, Unicode, SetValue, ações, cliques, rolagem, arrasto e erros da CLI.
O PNG da própria janela (2880 pixels de largura, escala fracionária do GNOME)
foi copiado por SSH e inspecionado visualmente.

Conferido no Windows 11 em VM VirtualBox no mesmo host: o instalador gerou e
instalou o app e a CLI. Na janela do app instalado passaram 40 consultas,
inspeção, foco, Unicode, SetValue, ações, cliques, teclado, rolagem e arrasto.
O PNG da própria janela (1024 × 697) foi inspecionado visualmente.
