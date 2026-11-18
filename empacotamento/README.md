# Empacotamento

> *"Não precisa de .dmg e appveyor!"* — dono, 27/set/2026.

O RPM local do Fedora e o MSIX do Windows são opções para distribuir o app pronto.
O instalador tradicional de macOS, Windows e Linux instala e se atualiza **compilando
o `main`** pelo [`scripts/instalar-vintagelightbox-gpui.cmd`](../scripts/instalar-vintagelightbox-gpui.cmd).
Naquele dia o próprio dono confirmou o caminho nos três sistemas: a 0.1.25 chegou ao Mac e ao Fedora
dele pelo script, antes de o pacote do macOS terminar de compilar, e sem conta de desenvolvedor
Apple o `.dmg` saía sem notarização (o macOS barrava a primeira abertura).

Saíram junto: o `.dmg` (`scripts/empacotar.sh`, `empacotar.ps1`, `packager.toml`, `macos/`), a
publicação no R2 (`scripts/lancar-local.sh`, `montar-manifesto.py`, `make publicar`), o workflow de
instaladores do GitHub Actions e o AppVeyor (`appveyor.yml`). Quem precisar do registro de como
eram, e por que foram desenhados assim, encontra no histórico do git antes de 27/set/2026.

## Lançar

1. Subir a versão num commit só, com **dois arquivos**: `Cargo.toml` (`[workspace.package]
   version`) e `docs/novidades.json` (a mesma `versao` e o texto para o operador do balcão). Um
   teste (`cargo test -p ui-gpui --lib novidades`) prende um ao outro.
2. `make producao` no `recordarfotos-e-commerce`: leva o `dev` ao `main` nos três projetos. Os
   apps leem o `docs/novidades.json` do `main`, avisam, e o "Atualizar" recompila.

O roteiro é a skill `lancar-o-app-desktop`, no e-commerce.

🚨 **O commit da versão é o último commit de código.** Quem compilou a 0.1.N não recompila outra
0.1.N: uma correção que entra depois não chega a ninguém. Suba outra versão.

**Não há volta de versão.** O app só atualiza para versão maior. Uma versão ruim se corrige com
outra, maior.

## O que ficou nesta pasta, e por quê

| Arquivo | Quem usa | Por que não sai |
|---|---|---|
| `icones/` | o app (`build.rs`, bandeja, menu) e o instalador (`.icns`, PNG do Linux) | compilado em todo balcão |
| `chave-publica.txt` | `atualizacao/porta.rs` (`include_str!`) | sem ela o app não compila; e o caminho do pacote, que o app ainda tem para os `.dmg` antigos, confere assinatura com ela |
| `enderecos-de-atualizacao.txt` | `atualizacao/porta.rs`, compilada no app | os mesmos apps ainda perguntam nesses endereços; **só se acrescenta** |

## 🚨 O que não pode acontecer com quem já tem o app

Ainda existem apps instalados pelo `.dmg`. Eles consultam o `latest.json` (R2 `vintagelightbox`,
depois o `docs/latest.json` do GitHub Pages), que ficou na **0.1.24**. Sem pacote novo, o app deles
roda o instalador e compila (`atualizacao/compilar.rs`) — e a partir daí vira uma instalação
compilada. Para isso continuar funcionando:

| Não fazer | O que acontece |
|---|---|
| Apagar o bucket `vintagelightbox` ou desligar o acesso público | o app cai no Pages; se o Pages também falhar, o `.dmg` antigo não descobre a versão nova pelo manifesto (ainda lê o `novidades.json`) |
| Apagar `docs/latest.json` ou `docs/novidades.json` | o Pages é o espelho de quem não alcança o R2 ou o `raw` do GitHub |
| Tirar endereço de `enderecos-de-atualizacao.txt` ou de `novidades::ENDERECOS` | os apps já instalados só conhecem os endereços com que foram compilados |
| Trocar `chave-publica.txt` ou perder `~/.vintagelightbox/atualizacao.key` | se um dia o pacote voltar, toda atualização vira "assinatura inválida" |

## O `.rpm` do Fedora

O [`scripts/empacotar-rpm.sh`](../scripts/empacotar-rpm.sh) monta um RPM local
para **x86_64**, a partir do binário release do `ui-gpui`. Ele inclui o executável,
o atalho do menu, ícones e a licença. Os recursos da interface são embutidos pelo
`build.rs`; modelos opcionais de IA não são incluídos no pacote.

Em uma máquina Fedora de build, instale `rpm-build`, `desktop-file-utils`, Rust e
as dependências de desenvolvimento descritas em `scripts/instalar-vintagelightbox-gpui.cmd`.
A compilação pode gerar ou atualizar `Cargo.lock` e precisa de rede para baixar
crates e dependências Git que ainda não estejam no cache. Execute na raiz do repositório:

```bash
./scripts/empacotar-rpm.sh
# Opcional: usar um executável Linux x86_64 já compilado:
./scripts/empacotar-rpm.sh --binario /caminho/para/ui-gpui
```

O resultado fica em `dist/fedora/`. Na máquina Fedora de destino:

```bash
sudo dnf install ./dist/fedora/vintagelightbox-<versao>-1.fc<versao_fedora>.x86_64.rpm
vintagelightbox-gpui
sudo dnf remove vintagelightbox
```

Substitua os campos pelo nome real do arquivo. Para atualizar um RPM local,
instale a versão mais recente com o DNF. A remoção não apaga dados do usuário.
O RPM não configura repositório: atualizações pelo DNF exigem um repositório
configurado separadamente. O app instalado em `/usr/bin/vintagelightbox-gpui`
não executa o atualizador que recompila outra instalação; a verificação dentro
do app orienta a usar o Fedora.

Na máquina de testes Fedora, use uma cópia de trabalho separada em
`~/oficina/trabalho/` e a trava `~/oficina/.trava-compilacao` para evitar outra
compilação pesada em paralelo. O script respeita `CARGO_TARGET_DIR`.

O pacote é gerado com o binário da máquina de build. Teste instalação e execução em cada versão do
Fedora que se pretende suportar. Também teste GPU, Wayland/X11, abertura de RAW,
câmera, impressão e recursos opcionais no ambiente de destino. A instalação
prévia em `~/.local` pode prevalecer no menu e deve ser tratada separadamente.

## O `.msix` do Windows

O [`scripts/empacotar-msix.ps1`](../scripts/empacotar-msix.ps1) gera um pacote
**x64** com o app, a CLI, as três DLLs do MinGW, ícones e licença.
O Windows disponibiliza a CLI como `vintagelightbox-vlb.exe`. No pacote de
teste, o comando é `vintagelightbox-vlb-testes.exe`.
Execute em Windows com o Windows SDK (`makeappx`, `signtool` e `fxc`), Rust
`stable-x86_64-pc-windows-gnu` e MSYS2 preparados pelo instalador tradicional.
O script não instala ferramentas nem o pacote. A saída fica em `dist/windows/`,
com metadados e SHA-256 em um JSON ao lado do MSIX.

Para compilar e assinar com um certificado de assinatura de código que já esteja
em `Cert:\CurrentUser\My`, com chave privada:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\empacotar-msix.ps1 -Certificado <thumbprint>
```

O Publisher do manifesto vem do Subject do certificado. Preserve essa identidade
nas próximas versões. O certificado de distribuição precisa ser confiável nas
máquinas de destino; o script não compra nem provisiona uma assinatura pública.
Veja a [documentação de assinatura da Microsoft](https://learn.microsoft.com/en-us/windows/msix/package/sign-app-package-using-signtool).

Para a VM de testes, use `-CertificadoDeTeste`. Isso cria/reutiliza um certificado
local e a identidade separada `RecordarFotos.VintageLightbox.Testes`. A chave
privada permanece no repositório de certificados; somente o `.cer` público sai
junto do pacote. Esse certificado não serve como assinatura pública de produção.

```powershell
# Reaproveitar uma compilação otimizada da mesma versão do Cargo.toml:
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\empacotar-msix.ps1 `
  -Binario C:\build\ui-gpui.exe -Cli C:\build\vlb.exe -CertificadoDeTeste
# A pasta de DLLs padrão é C:\msys64\mingw64\bin; altere com -PastaDll.
# Para compilar com menos memória, pode usar -Perfil instalador.

# Somente na VM de testes, PowerShell como administrador:
Import-Certificate -FilePath .\dist\windows\RecordarFotos.VintageLightbox.Testes.cer `
  -CertStoreLocation Cert:\LocalMachine\TrustedPeople
# Na conta do operador de teste:
Add-AppxPackage -Path .\dist\windows\RecordarFotos.VintageLightbox.Testes-<versao>-x64.msix
Get-AppxPackage -Name RecordarFotos.VintageLightbox.Testes
# Abra VintageLightbox (Teste MSIX) pelo menu Iniciar.
Get-AppxPackage -Name RecordarFotos.VintageLightbox.Testes | Remove-AppxPackage
```

A versão do workspace `0.1.200` vira `0.1.200.0`; `-VersaoPacote` permite informar
os quatro componentes para ensaiar atualização na VM. Cada componente deve estar
entre 0 e 65535. Instale o MSIX mais recente com `Add-AppxPackage` para atualizar,
mantendo nome, Publisher e arquitetura. Encerre o app antes da atualização.
O pacote é gerenciado pelo Windows; o app não tenta recompilar outra instalação.
Ainda não há feed `.appinstaller` nem atualização automática do MSIX.

O manifesto desabilita a virtualização de escrita no AppData, para manter o
catálogo e os dados do app após a remoção (veja [virtualização no MSIX](https://learn.microsoft.com/en-us/windows/msix/desktop/flexible-virtualization)).
Use `VLB_CATALOG` e `VLB_CONTROLE` em pastas exclusivas durante os testes. Não
inclua catálogo, credenciais nem modelos opcionais de IA no pacote. A IA opcional
continua precisando dos pré-requisitos do seu backend, como o Visual C++ Runtime.
A VM permite validar instalação e abertura com renderização por software;
GPU, câmera e impressão precisam de teste no hardware de destino.

## Trocar o ícone

Troque `icones/icone-mestre.png` (quadrado, mínimo 1024×1024) e rode `./scripts/gerar-icones.sh`. O
`.icns` (10 medidas), o `.ico` (6), os PNGs do Linux e o do site saem todos dele.

⚠️ O script **recusa** mestre não-quadrado ou menor que 1024: ampliar entrega um ícone borrado
justamente no tamanho em que ele mais aparece.
