# Empacotamento — o que sobrou, e por que não se apaga

> *"Não precisa de .dmg e appveyor!"* — dono, 27/set/2026.

**Não há mais pacote.** Todo balcão — macOS, Windows e Linux — instala e se atualiza **compilando
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

## 🐧 O pacote pronto do Linux (issue #37)

> *"liberar instaladores já compilados para os usuários não ficarem esperando tanto"* — dono, 18/11/2026.

**O pacote é o primeiro caminho; a compilação é a segunda.** O Linux (piso: **Fedora 44**) é o
primeiro sistema a ter isso. Windows e macOS seguem compilando até as suas etapas da issue.

| Peça | Onde | O que faz |
|---|---|---|
| `scripts/empacotar-pacote.sh` | roda na **fedora** | compila em `release` (com `strip` e `--remap-path-prefix`) e gera `vintagelightbox-<versão>-linux-x86_64.tar.gz` + `.glibc`; recusa binário que peça glibc maior que a do Fedora 44 |
| assinatura | **só no Mac do dono** | `minisign -S` com a chave do app (ID `33D71240F44E3949`); a chave privada nunca sai de lá |
| `scripts/manifesto-do-pacote.sh` | Mac do dono | escreve o `latest-linux.json` (formato do `cargo_packager_updater` + o campo `glibc`) |
| `docs/instalar.sh` | cliente (`curl … \| sh`) | baixa, **confere a assinatura**, abre, prova `--versao`, instala em `~/.local` |
| `scripts/testar-instalar-linux.sh` | fedora | ensaio com chave de teste: instala, atualiza e recusa pacote adulterado, chave errada, versão divergente, glibc baixa |

**Onde instala:** `~/.local/bin/vintagelightbox-gpui` (com o `.anterior` ao lado), `~/.local/share/applications/`
e `~/.local/share/icons/`. É o **mesmo lugar do instalador por compilação**, de propósito: o app
(`atualizacao::compilar::binario_instalado`) procura e troca o binário ali. A issue sugeria uma pasta
`~/.local/share/vintagelightbox/` com link; um link teria de ser refeito a cada troca feita pelo app, e
o `.anterior` ficaria em outro lugar.

**Como o cliente confere a assinatura sem `minisign`:** o minisign é Ed25519 sobre o BLAKE2b-512 do
arquivo, e o `openssl` do Fedora faz as duas contas (`openssl dgst -blake2b512` + `openssl pkeyutl
-verify -rawin`). A chave pública vem **dentro do `instalar.sh`** (cópia de `chave-publica.txt`; o
ensaio confere que são idênticas) e a assinatura vem no manifesto. Nada é extraído nem executado antes
de a conferência devolver verde.

**Quando cai para a compilação** (uma vez, sem laço): sem `latest-linux.json` no R2; sem pacote para a
arquitetura; glibc da máquina menor que a do pacote (ou musl); falta `openssl`/`curl`; download
falhou; assinatura inválida; pacote que não abre; `--versao` diferente da versão anunciada.

**Medido em 18/11/2026, na fedora (Fedora 44):** `ldd --version` = **glibc 2.43**; o binário da 0.1.200
(`release`, 112 MB) pede no máximo **`GLIBC_2.43`** (`objdump -T <bin> | grep GLIBC_`). Ou seja, o piso do
Fedora 44 é de fato o mínimo: qualquer Linux com glibc menor cai para a compilação. O
`empacotar-pacote.sh` **recusa** um binário que peça mais que 2.43, e o `latest-linux.json` leva a glibc
do pacote para o `instalar.sh` decidir antes de baixar.

**Caminhos da máquina no binário:** o primeiro build deixava `/home/<usuário>/.cargo/git/checkouts/…`
(dependências por git, como o `nucleo-da-foto`) e a pasta `build/` do `target`; só o `registry/src`
estava mapeado. O script agora mapeia `$HOME`→`/h`, `registry/src`→`/r`, `git/checkouts`→`/g` e o
projeto→`/f`. Conferido no binário recompilado: nenhum `/home/…` sobrou (resta `/h/…/rules.rs`, do
`html5ever`, sem nome de usuário).

### Publicar um pacote do Linux (Passo 3 da issue, no Mac do dono)

1. Na fedora: `source ~/oficina/bin/ambiente.sh && flock ~/oficina/.trava-compilacao scripts/empacotar-pacote.sh`
   (depois de `make lint`, `make testar` e `make e2e` verdes no `dev`). Sai em `dist/pacote/`.
2. Trazer o `.tar.gz` e o `.glibc` para o Mac (`scp fedora:…/dist/pacote/vintagelightbox-* .`).
3. Assinar: `minisign -S -s <chave privada do ID 33D71240F44E3949> -m vintagelightbox-<v>-linux-x86_64.tar.gz`
   (ou `cargo packager signer sign`). **Conferir** que a chave pública de `chave-publica.txt` aceita.
4. Subir o `.tar.gz` para o bucket `vintagelightbox` do R2 (prefixo da versão) e rodar
   `scripts/manifesto-do-pacote.sh <pacote> <pacote>.minisig <url-do-r2>` para gerar o `latest-linux.json`;
   subir **o manifesto por último**, depois do pacote.
5. Só então lançar a versão (o aviso em `docs/novidades.json` no `main` sai **depois** do pacote publicado).
6. 🚨 **Nunca tocar** em `latest.json` (congelado na 0.1.24, só macOS), `chave-publica.txt` nem
   `enderecos-de-atualizacao.txt`.

### O que ainda falta da issue #37 para o Linux

- **Passo 0:** a chave privada do ID `33D71240F44E3949` não está no Mac do dono. Sem ela, nada se assina
  nem se publica. Não gerar par novo sem o "sim" dele (trocar o par exige lançar uma versão por compilação
  antes).
- **Passo 4:** o app ainda não baixa o pacote para se atualizar (`atualizacao/porta.rs`, `novidades.rs`,
  `faixa.rs`); hoje só o instalador usa o pacote. PR à parte.
- **Lançamento:** acrescentar o passo "pacote publicado antes do aviso" à skill `lancar-o-app-desktop`
  (no e-commerce) e conferir o que o `make producao` precisa esperar.

## O `.rpm` do Fedora

[`scripts/empacotar-rpm.sh`](../scripts/empacotar-rpm.sh) continua: gera um `.rpm` na própria
máquina Fedora, para instalar em outros computadores sem compilar em cada um. Não é publicado em
lugar nenhum.

## Trocar o ícone

Troque `icones/icone-mestre.png` (quadrado, mínimo 1024×1024) e rode `./scripts/gerar-icones.sh`. O
`.icns` (10 medidas), o `.ico` (6), os PNGs do Linux e o do site saem todos dele.

⚠️ O script **recusa** mestre não-quadrado ou menor que 1024: ampliar entrega um ícone borrado
justamente no tamanho em que ele mais aparece.
