---
name: maquina-de-testes-fedora
description: Use a máquina Fedora de testes via Tailscale para compilar, empacotar e validar o VintageLightbox em Linux ou na VM Windows 11. Acione para RPM, MSIX, Windows Installer, GNOME/Wayland, GPU, testes de instaladores ou builds longos na oficina.
---

# Máquina de testes Fedora

Use o Fedora da oficina para builds e testes Linux e a VM Windows 11 para executar e validar pacotes Windows. A oficina faz o papel do CI para esses testes.

## Verificar acesso

1. Confira o host local com `hostname`. Se já estiver no Fedora, execute os comandos diretamente.
2. No Mac, tente `ssh -o BatchMode=yes -o ConnectTimeout=10 fedora 'hostname; cat /etc/fedora-release'`.
3. Se falhar, verifique o alias com `ssh -G fedora` e conectividade com `tailscale status` ou o aplicativo Tailscale. O guia local `../AMBIENTE-DE-TESTES.md` registra o MagicDNS e o diagnóstico. Preserve a verificação da chave SSH. Se o executor bloquear rede ou a VPN estiver indisponível, informe a falha observada e prossiga com verificações locais que façam sentido.
4. Para confirmar acesso à VM, execute `ssh -o BatchMode=yes -o ConnectTimeout=10 win11 'whoami'`. Do Fedora, também pode usar o alias `win11`. Uma falha na VM não implica que o Fedora esteja offline.

## Compilar e conferir Linux

- Trabalhe em uma cópia isolada sob `~/oficina/trabalho/<tarefa>`. A bateria atualiza os clones em `~/oficina/repos`; mantenha suas alterações fora deles.
- Carregue `~/oficina/bin/ambiente.sh`, configure `CARGO_TARGET_DIR` em `~/oficina/target/<tarefa>` e use `flock ~/oficina/.trava-compilacao <comando>` para serializar builds pesados. Registre o log e o código de saída.
- No VintageLightbox, gere o RPM com `./scripts/empacotar-rpm.sh` e recolha o artefato de `dist/fedora/`. Confira metadados e conteúdo com `rpm -qip` e `rpm -qlp`. Valide a instalação com DNF em ambiente isolado e diferencie o teste de pacote do teste gráfico em GNOME/Wayland. Use catálogo e fotos de teste.
- Para acompanhar a bateria, leia `~/oficina/relatorios/ultimo/resumo.md` e consulte o log do passo relevante.

## Validar Windows

- A VM do VirtualBox se chama `Windows 11 Testes`. Use PowerShell via `ssh win11` ou `~/oficina/bin/win` no Fedora. Confira o estado com `VBoxManage list runningvms`.
- Antes de suspender, desligar ou restaurar a VM, carregue `~/oficina/bin/ambiente.sh` e verifique `vm_em_uso`. Se ocupada, preserve o estado. Coordene testes longos com o marcador existente `~/oficina/.vm-em-uso` sem sobrescrever nem remover o marcador de outra sessão.
- Para ensaiar o instalador legado em clone isolado, use `~/oficina/bin/testar-instalador-windows.sh <ref>` e leia os relatórios. Para um novo pacote, transfira o artefato para um ambiente de teste e valide instalação limpa, atualização e abertura na sessão gráfica. A VM usa renderização por software. Teste GPU e desempenho em hardware adequado.
- Preserve instalações e catálogos de produção e arquivos de outras sessões. Recolha os artefatos e limpe os arquivos descartáveis criados pela tarefa. Evite incluir credenciais em comandos, logs ou relatórios.
