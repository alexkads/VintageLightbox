---
name: maquina-de-testes-fedora
description: Acesse a oficina Fedora via Tailscale e SSH para compilar, empacotar e validar o VintageLightbox no Fedora ou na VM Windows 11 do libvirt. Use para RPM, MSIX, testes de instaladores, Wayland e builds longos na oficina.
---

# Máquina de testes Fedora

A oficina roda os builds do VintageLightbox e hospeda as VMs `vlb-fedora44` e
`vlb-win11` em QEMU/KVM. O Virt-Manager é a interface opcional do mesmo libvirt.
Faça a preparação, a instalação e os testes remotamente por SSH. A sessão
gráfica do host não é requisito.

## Acesso

- No Mac, use `ssh -o BatchMode=yes -o ConnectTimeout=10 fedora`. Se o MagicDNS
  falhar, consulte `tailscale status` e use `ssh -o HostName=<ipv4-atual> fedora`
  com o IPv4 do nó de testes. Preserve a verificação da chave SSH.
- No Fedora, liste as VMs com `sudo -n virsh -c qemu:///system list --all`.
  Use `virsh start <nome>` e `virsh shutdown <nome>` com o mesmo privilégio.
  Antes de desligar uma VM compartilhada, consulte o marcador
  `~/oficina/.vm-em-uso` e, para o Windows, a função `vm_em_uso` em
  `~/oficina/bin/ambiente.sh`. Preserve sessões e marcadores de terceiros.
- Use `ssh win11` do Mac ou do Fedora, ou `~/oficina/bin/win 'whoami'` no
  Fedora. No Mac, o alias usa o Fedora como salto para a rede privada da VM.
  Se precisar ver o console da VM, use
  `~/oficina/bin/win --tela arquivo.png` ou `virsh screenshot`.

## Build e RPM

- Use uma cópia isolada em `~/oficina/trabalho/<tarefa>` e
  `CARGO_TARGET_DIR=~/oficina/target/<tarefa>`. Carregue
  `~/oficina/bin/ambiente.sh` e serialize builds pesados com
  `flock ~/oficina/.trava-compilacao <comando>`. Preserve os clones em
  `~/oficina/repos`, que são atualizados pela bateria.
- Gere o RPM com `./scripts/empacotar-rpm.sh`. Recolha o artefato em
  `dist/fedora/` e confira `rpm -qip` e `rpm -qlp`. Na VM, instale com DNF,
  confira `--versao`, abra com `VLB_CATALOG` e `VLB_CONTROLE` apontando para
  pastas descartáveis, capture a janela e confirme a remoção.
- Em teste Wayland remoto, o GPUI precisa de dispositivos de entrada. O ensaio
  validado usa Weston sobre Xvfb dentro da VM. Weston com backend headless
  sozinho não fornece o seat que o GPUI espera. Esse ensaio usa renderização
  por software. Para medir GPU, use hardware adequado.
- Se a VM resolver nomes, mas o DNF não conseguir sair, um túnel SOCKS
  temporário pela conexão SSH do Fedora para a VM permite verificar e concluir
  a instalação sem mudar firewall, DNS ou a VPN do host.

## Build e MSIX

- A `vlb-win11` tem Windows 11 Enterprise Evaluation x64, Secure Boot, TPM 2.0,
  SSH, Windows SDK, Rust GNU e MSYS2. Use uma cópia isolada dentro do convidado.
  Ao criar tar no macOS, exclua metadados AppleDouble (`COPYFILE_DISABLE=1`
  e `--exclude '._*'`). O SQLx interpreta esses arquivos como migrações.
- Gere com `scripts/empacotar-msix.ps1`. Para instalação na VM, use a
  identidade e o certificado de teste. Confira assinatura, instalação limpa,
  abertura, atualização e remoção com dados de teste. Na oficina,
  `Add-AppxPackage` diretamente na sessão OpenSSH falhou ao inicializar o PLM
  (`0x80070005`). Dispare a instalação por uma tarefa interativa na sessão
  do usuário do convidado, que pode ser criada e iniciada pelo SSH. Use a mesma
  sessão para abrir o executável e confira a janela por screenshot ou pela CLI
  `vlb`.
- Recolha o pacote, o log e o código de saída. Os relatórios da oficina ficam
  em `~/oficina/relatorios/`. Use uma pasta por tarefa e mantenha credenciais
  e catálogos de produção fora dos testes.
