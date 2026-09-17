# Instalar o VintageLightbox (Zed GPUI)

O **VintageLightbox (Zed GPUI)** é o editor nativo do projeto: as telas são desenhadas em Rust (GPUI),
com o motor de revelação na GPU, o catálogo local, a importação do cartão da câmera, a tela do
cliente no segundo monitor e o caixa do balcão.

> **Há dois apps, e dois instaladores.** Este guia é do **GPUI**, e o arquivo dele termina em
> **`-gpui.cmd`**. O app do balcão, o **VintageLightbox (Tauri)**, se instala com
> `instalar-vintagelightbox-tauri.cmd`: veja [INSTALAR-TAURI.md](INSTALAR-TAURI.md). **Na dúvida, o
> do balcão é o Tauri.** Os dois convivem na mesma máquina.

Instalado, ele aparece como:

- **macOS**: **VintageLightbox (Zed GPUI)** (`VintageLightbox (Zed GPUI).app`), com o mesmo
  identificador dos pacotes prontos (`.dmg`). Um `VintageLightbox.app` antigo não é apagado;
- **Windows e Linux**: **VintageLightbox (Zed GPUI)**, para não se confundir com o Tauri.

O título da janela e a barra de menus também dizem **VintageLightbox (Zed GPUI)**, como o outro
app diz **VintageLightbox (Tauri)**.

Não há instalador pronto. **Cada máquina compila o próprio app**, com **um arquivo só**, que funciona
em Windows, Linux e macOS e instala sozinho quase tudo o que falta (no macOS, o Xcode é o único que
você instala à mão):

**[instalar-vintagelightbox-gpui.cmd](https://github.com/alexkads/VintageLightbox/releases/download/instalador-tauri/instalar-vintagelightbox-gpui.cmd)**

---

## Windows

1. **[Clique aqui para baixar o instalador](https://github.com/alexkads/VintageLightbox/releases/download/instalador-tauri/instalar-vintagelightbox-gpui.cmd)**.
   Se o navegador disser que o arquivo pode ser perigoso, escolha **Manter**.
2. Na pasta **Downloads**, dê **dois cliques** em `instalar-vintagelightbox-gpui.cmd`.
3. Se o Windows mostrar *"O Windows protegeu o computador"*, clique em **Mais informações** e depois
   em **Executar assim mesmo**. Isso só acontece na primeira vez.
4. Se o Windows perguntar se permite que um programa faça alterações, clique em **Sim**: são as
   ferramentas que o instalador põe na máquina.
5. Espere. A janela mostra o andamento, e **a primeira vez leva de 15 a 40 minutos**.
6. No fim, aperte qualquer tecla. O app aparece no **Menu Iniciar** como
   **VintageLightbox (Zed GPUI)**.

O instalador cuida sozinho de:
- **Windows SDK**, de onde vem o `fxc.exe`, que compila os shaders do GPUI;
- **Rust**, na versão `-gnu` (o Visual Studio não é preciso);
- **MSYS2** com o compilador `g++`, a `libclang` e o `windres`.

O **winget** instala o Windows SDK e o MSYS2; o **rustup** instala o Rust; o **pacman** do MSYS2
instala o compilador, a libclang e o `windres`. Se o Windows não tiver o winget (Windows 10 antigo ou
LTSC), o instalador avisa e indica onde baixá-lo.

## macOS

1. Instale o **[Xcode](https://apps.apple.com/app/xcode/id497799835)** pela App Store. É grátis.
   **Abra-o uma vez** depois de instalado.
2. Abra o **Terminal**: aperte **⌘ Espaço**, digite `Terminal` e aperte **Enter**.
3. Cole esta linha e aperte **Enter**:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox-gpui.cmd | sh
   ```

4. Quando pedir, digite **a senha do Mac** (ela não aparece enquanto você digita). Ela serve para
   apontar as ferramentas para o Xcode e aceitar a licença dele.
5. Espere. **A primeira vez leva de 15 a 40 minutos**, mais alguns minutos para baixar o componente
   Metal.
6. Pronto: o app fica em **Aplicativos**, como **VintageLightbox**. Ele abre no primeiro clique, sem
   aviso de segurança.

🚨 **Por que o Xcode inteiro.** O GPUI compila os shaders Metal durante a compilação, e o compilador
`metal` não vem nas Command Line Tools: vem no Xcode, e a partir do Xcode 26 é um componente que se
baixa à parte. O instalador faz o resto sozinho (aponta o `xcode-select`, aceita a licença e baixa o
componente), mas **o Xcode só se instala pela App Store**. O app do balcão (Tauri) não precisa dele.

O instalador cuida sozinho de:
- apontar as ferramentas para o Xcode, aceitar a licença e baixar o **componente Metal**;
- o **Rust**, na sua pasta pessoal e sem senha.

## Linux

1. Abra o **Terminal**.
2. Cole esta linha e aperte **Enter**:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox-gpui.cmd | sh
   ```

3. Digite a sua senha quando ela for pedida. Ela serve para instalar as bibliotecas do sistema.
4. Espere. **A primeira vez leva de 15 a 40 minutos.**
5. Pronto: o app aparece no menu de aplicativos como **VintageLightbox (Zed GPUI)**.

O instalador cuida sozinho de (pelo `apt`, `dnf` ou `pacman`, pedindo a senha de administrador):
- o compilador e a `libclang`;
- as bibliotecas que o GPUI abre: X11, Wayland, xkbcommon, fontconfig, freetype, ALSA, OpenSSL, D-Bus,
  libsecret e o **Vulkan**, com o driver Mesa (sem um driver Vulkan a janela não abre, mesmo com tudo
  compilado);
- o Rust. Um Rust antigo instalado pela distribuição é deixado de lado, e o instalador põe o `rustup`
  à frente.

Sem `curl`, instale-o antes (`sudo apt install curl` ou `sudo dnf install curl`).

### No Fedora

- **Workstation (GNOME):** o GNOME não mostra ícone de bandeja sem a extensão *AppIndicator*. O
  instalador a instala pelo `dnf` e a liga; **saia e entre de novo na sessão** para o ícone
  aparecer. Sem ela o app funciona, mas minimizado só volta pelo Alt+Tab.
- **Silverblue, Kinoite ou Bazzite:** o sistema é imutável e recusa `dnf install`. O instalador
  para e mostra um `sudo rpm-ostree install …`: rode-o, reinicie e rode o instalador de novo.
- Com placa NVIDIA, o Vulkan vem do driver da NVIDIA (`akmod-nvidia`, do RPM Fusion).
- A primeira instalação baixa cerca de 2 GiB de pacotes; a compilação usa mais uns 10 GiB de
  disco. O instalador abre uma compilação por 4 GiB de memória; com 8 GiB ou menos, feche os
  outros programas antes. Se ainda assim aparecer `signal: 9` (`SIGKILL`), repita com `… | CARGO_BUILD_JOBS=1 sh`.

---

## Atualizar

**Faça de novo o mesmo passo da instalação:** dois cliques no Windows, ou o mesmo comando no Linux e
no macOS. O cache reduz o trabalho nas próximas execuções; o tempo depende da máquina, da rede e do
que mudou.

O download é validado antes de substituir o código anterior, e, se o conteúdo não mudou, o
instalador preserva os arquivos e suas datas para não recompilar à toa.

O app GPUI também tem **atualização automática**, que compara a versão instalada com a publicada
(`docs/latest.json`). Compilado da branch `dev`, ele costuma estar à frente da publicada e não vê
atualização; para acompanhar a `dev`, repita a instalação.

## Opções (para quem sabe o que está fazendo)

| O que | Linux e macOS | Windows (antes dos dois cliques, num `cmd`) |
|---|---|---|
| Só mostrar o que faria | `… \| sh -s -- --seco` | `set VLB_SECO=1` |
| Outra versão (branch ou tag) | `… \| sh -s -- --versao v0.1.2` | `set VLB_VERSAO=v0.1.2` |
| Outra pasta de instalação | `… \| sh -s -- --destino ~/Apps` | `set VLB_DESTINO=D:\Apps\VintageLightbox` |
| Caminho do `fxc.exe` já instalado | — | `set GPUI_FXC_PATH=C:\...\fxc.exe` |

Ler antes de rodar é legítimo:

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox-gpui.cmd -o instalar.cmd
less instalar.cmd
sh instalar.cmd
```

### O endereço antigo e o `instalar.sh`

- `instalar-vintagelightbox.cmd`, o nome antigo, **instala o Tauri**. Para ele instalar o GPUI, use
  `VLB_APP=gpui` (`curl … | VLB_APP=gpui sh`, ou `set VLB_APP=gpui` no `cmd`). Para instalação nova,
  use o `-gpui.cmd` direto.
- `https://alexkads.github.io/VintageLightbox/instalar.sh` continua existindo: só macOS, compila a
  **versão publicada** (e não a `dev`) e não prepara o Xcode sozinho. O `-gpui.cmd` o substitui nos
  três sistemas.

### Como um arquivo só roda nos três sistemas

É o mesmo desenho do instalador do Tauri, descrito em
[INSTALAR-TAURI.md](INSTALAR-TAURI.md#como-um-arquivo-só-roda-nos-três-sistemas): o `cmd` roda o
bloco do topo, que chama o PowerShell do meio do arquivo; o `sh` pula esses dois blocos e roda o
resto. Os dois instaladores são **gerados** de `scripts/instalador-modelo.cmd.in` por
`python3 scripts/gerar-instaladores.py`: edite o modelo, nunca o `.cmd`.

## Onde as coisas ficam

| | macOS | Linux | Windows |
|---|---|---|---|
| O app | `/Applications/VintageLightbox (Zed GPUI).app` | `~/.local/bin/vintagelightbox-gpui` | `%LOCALAPPDATA%\Programs\VintageLightbox-GPUI` |
| Cache do compilador (alguns GiB) | `~/.vintagelightbox/target-gpui` | `~/.vintagelightbox/target-gpui` | `%USERPROFILE%\.vintagelightbox\target-gpui` |
| Código baixado (descartável) | `~/.vintagelightbox/fonte-gpui` | `~/.vintagelightbox/fonte-gpui` | `%USERPROFILE%\.vintagelightbox\fonte-gpui` |

Apagar `target-gpui` e `fonte-gpui` é seguro: a próxima atualização só demora mais. As pastas
`-tauri` ao lado são do outro app.

**Desinstalar**: no macOS, `VintageLightbox` para o Lixo; no Windows, a pasta acima e o atalho
**VintageLightbox (Zed GPUI)** do Menu Iniciar; no Linux, `~/.local/bin/vintagelightbox-gpui`,
`~/.local/share/applications/vintagelightbox-gpui.desktop` e o ícone de mesmo nome.

## Quando algo dá errado

| Mensagem | O que fazer |
|---|---|
| `falta o Xcode` (macOS) | Instalar o Xcode pela App Store, abrir uma vez e repetir. Se queria o app do balcão, use o [instalador do Tauri](INSTALAR-TAURI.md), que não precisa dele |
| `o compilador Metal continua faltando` (macOS) | Rodar, um de cada vez: `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`, `sudo xcodebuild -license accept` e `xcodebuild -downloadComponent MetalToolchain`. Depois repetir |
| `o fxc.exe continua faltando` (Windows) | Instalar o [Windows SDK](https://developer.microsoft.com/windows/downloads/windows-sdk/), ou definir `GPUI_FXC_PATH` com o caminho do `fxc.exe`, e repetir |
| `o windres do MinGW continua faltando` (Windows) | No terminal do MSYS2, rodar `pacman -S mingw-w64-x86_64-binutils`. Depois repetir |
| `o winget ... não existe nesta máquina` (Windows) | Instalar o [Instalador de Aplicativo](https://apps.microsoft.com/detail/9NBLGGH4NNS1) pela Microsoft Store e repetir |
| `o g++ do MinGW continua faltando` (Windows) | Instalar o [MSYS2](https://www.msys2.org) e, no terminal dele, rodar `pacman -S mingw-w64-x86_64-gcc`. Depois repetir |
| `a libclang continua faltando` | No Windows, no terminal do MSYS2: `pacman -S mingw-w64-x86_64-clang`. No Linux: instalar `clang` e `libclang-dev` (ou os equivalentes). Depois repetir |
| `não reconheci o gerenciador de pacotes` (Linux) | Instalar à mão os pacotes listados na mensagem e repetir |
| `a instalação dos pacotes falhou` (Linux) | Esta conta não pode instalar programas: pedir a quem administra a máquina para rodar o comando mostrado logo acima, e repetir |
| `o Rust continua ausente ou anterior ao 1.89` | Rodar `rustup update stable` e repetir |
| A janela não abre (Linux) | Falta um driver Vulkan: instalar `mesa-vulkan-drivers` (ou o da sua placa de vídeo) |
| A compilação morre por falta de memória (`SIGKILL`) | Fechar outros programas e repetir; o instalador já compila só o binário do app |
| A compilação para por falta de espaço | Liberar alguns GiB e repetir |

## Validação do instalador

`python3 scripts/testar-instalador.py` cobre os dois instaladores e o endereço antigo sem rede, sem
compilar e sem instalar nada (os detalhes estão em [INSTALAR-TAURI.md](INSTALAR-TAURI.md#validação-do-instalador)).
Os testes não substituem instalar e abrir o app em máquinas reais: o duplo clique do Windows, o
PowerShell 5.1, o winget com o Windows SDK, o `windres` na compilação do manifesto, a preparação do
Xcode e as bibliotecas do Linux só se conferem lá.
