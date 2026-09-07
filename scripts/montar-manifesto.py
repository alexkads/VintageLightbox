#!/usr/bin/env python3
"""Monta o `latest.json` e a página, a partir do que o CI empacotou.

Roda no job `lancar` do `.github/workflows/instaladores.yml`, depois que as três
plataformas terminaram. Lê `dist/`, escreve `docs/` — que é de onde o GitHub Pages serve.

🔑 **É o que substituiu o `publicar.py`.** Aquele subia para o Supabase Storage e
   precisava da `service_role`; este não toca credencial nenhuma — os arquivos
   vão para o Releases do próprio repositório e o manifesto para o Pages. A
   mudança tirou a última chave sensível do caminho de lançamento (7/set/2026).

⚠️ **Roda no CI e roda na mão, e é de propósito.** Enquanto o Actions esteve
   bloqueado por cobrança (7/set/2026), o lançamento saiu daqui mesmo: gerar com
   `make mac`, rodar este script, commitar `docs/` e `gh release create`. O
   `docs/` é servido pelo Pages **direto do branch**, sem Actions no caminho.

⚠️ **O `latest.json` é estático, e é o app que compara as versões.** Não há
   endpoint decidindo `204`: o manifesto traz todas as plataformas, e o updater
   lê o `version`, confronta com a instalada e escolhe a entrada de `platforms`
   que corresponde à máquina dele. Plataforma ausente = "nada novo" para ela.
"""

import argparse
import json
import pathlib
import re
import sys
from datetime import datetime, timezone

RAIZ = pathlib.Path(__file__).resolve().parent.parent
DIST = RAIZ / "dist"
SITE = RAIZ / "docs"

# 🔑 **A ordem é prioridade**, e resolve o caso de haver `.exe` e `.msi` no mesmo
#    `dist/`: os dois disputam `windows-x86_64`. O NSIS vem antes porque é o
#    único dos dois que reabre o app sozinho depois de instalar.
#
# ⚠️ Os formatos são os quatro que o `cargo-packager-updater` sabe aplicar. O
#    `.dmg` e o `.deb` **não atualizam nada** — eles são para a primeira
#    instalação. Quem atualiza o macOS é o `.app.tar.gz`.
ATUALIZACAO = [
    (r"\.app\.tar\.gz$", ["macos-aarch64", "macos-x86_64"], "app"),
    (r"aarch64\.AppImage$", ["linux-aarch64"], "appimage"),
    (r"\.AppImage$", ["linux-x86_64"], "appimage"),
    (r"-setup\.exe$", ["windows-x86_64"], "nsis"),
    (r"\.msi$", ["windows-x86_64"], "wix"),
]

# O que a página oferece para baixar à mão, na ordem em que aparece.
DOWNLOADS = [
    (r"universal\.dmg$", "macOS", "Intel e Apple Silicon"),
    (r"aarch64\.dmg$", "macOS", "Apple Silicon"),
    (r"x86_64\.dmg$", "macOS", "Intel"),
    (r"-setup\.exe$", "Windows", "Instalador (.exe)"),
    (r"\.msi$", "Windows", "Pacote MSI"),
    (r"arm64\.deb$|aarch64\.deb$", "Linux", "ARM64 (.deb)"),
    (r"\.deb$", "Linux", "x86-64 (.deb)"),
    (r"aarch64\.AppImage$", "Linux", "ARM64 (AppImage)"),
    (r"\.AppImage$", "Linux", "x86-64 (AppImage)"),
]


def versao_do_workspace() -> str:
    texto = (RAIZ / "Cargo.toml").read_text()
    bloco = texto.split("[workspace.package]", 1)[1]
    m = re.search(r'^version\s*=\s*"([^"]+)"', bloco, re.M)
    if not m:
        sys.exit("❌ não achei a versão em [workspace.package] do Cargo.toml")
    return m.group(1)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--notas", default="", help="o texto que o app mostra ao avisar")
    ap.add_argument("--repo", required=True, help="dono/repositorio")
    ap.add_argument("--tag", default="", help="a tag do lançamento, se houver")
    args = ap.parse_args()

    versao = versao_do_workspace()
    tag = args.tag if args.tag.startswith("v") else f"v{versao}"
    base = f"https://github.com/{args.repo}/releases/download/{tag}"

    interessa = (".dmg", ".deb", ".AppImage", ".exe", ".msi", ".tar.gz")
    arquivos = sorted(p for p in DIST.rglob("*") if p.is_file() and p.name.endswith(interessa))
    if not arquivos:
        sys.exit("❌ dist/ não tem pacote nenhum")

    plataformas: dict[str, dict] = {}
    downloads: list[dict] = []
    for caminho in arquivos:
        nome = caminho.name
        for padrao, alvos, formato in ATUALIZACAO:
            if re.search(padrao, nome):
                sig = caminho.with_name(nome + ".sig")
                if not sig.exists():
                    print(f"   ⚠️  {nome} sem .sig — fica de fora da atualização")
                    break
                for alvo in alvos:
                    # A lista acima já é a ordem de preferência: não sobrescreve.
                    plataformas.setdefault(alvo, {
                        "signature": sig.read_text().strip(),
                        "url": f"{base}/{nome}",
                        "format": formato,
                    })
                break
        for padrao, sistema, arq in DOWNLOADS:
            if re.search(padrao, nome):
                downloads.append({
                    "sistema": sistema, "arquitetura": arq, "arquivo": nome,
                    "url": f"{base}/{nome}", "bytes": caminho.stat().st_size,
                })
                break

    agora = datetime.now(timezone.utc).isoformat(timespec="seconds")
    # O formato é o do `cargo-packager-updater`: `version`, `platforms`, e os
    # opcionais `notes` e `pub_date`. Nomes em inglês porque quem lê é o crate.
    manifesto = {
        "version": versao,
        "notes": args.notas,
        "pub_date": agora,
        "platforms": plataformas,
    }

    SITE.mkdir(exist_ok=True)
    (SITE / "latest.json").write_text(json.dumps(manifesto, indent=2) + "\n")
    # A página lê este, que tem o que o updater não precisa: tamanho e rótulo.
    (SITE / "downloads.json").write_text(
        json.dumps({"versao": versao, "publicado_em": agora, "notas": args.notas,
                    "downloads": downloads}, indent=2, ensure_ascii=False) + "\n")

    corpo = [f"**VintageLightbox {versao}**", ""]
    if args.notas:
        corpo += [args.notas, ""]
    corpo += ["| Sistema | Arquivo | Tamanho |", "|---|---|---|"]
    corpo += [f"| {d['sistema']} · {d['arquitetura']} | `{d['arquivo']}` | {d['bytes']/1e6:.0f} MB |"
              for d in downloads]
    corpo += ["", "Baixe em **https://alexkads.github.io/VintageLightbox/**", "",
              "⚠️ No macOS, na primeira abertura clique com o botão direito no",
              "aplicativo e escolha *Abrir* — ele não é distribuído pela App Store.", "",
              "---", "",
              "Feito pelo **Recordar Fotos Estúdio** — Gramado e Canela, RS."]
    (SITE / "notas-do-lancamento.md").write_text("\n".join(corpo) + "\n")

    print(f"\n▸ VintageLightbox {versao} · tag {tag}")
    print(f"   {len(plataformas)} plataforma(s) com atualização: {', '.join(sorted(plataformas)) or '(nenhuma)'}")
    print(f"   {len(downloads)} download(s): {', '.join(d['arquivo'] for d in downloads)}")
    if not plataformas:
        print("   ⚠️  nenhum pacote tinha .sig — ninguém recebe atualização automática")

    saida = pathlib.Path(__import__("os").environ.get("GITHUB_OUTPUT", "/dev/null"))
    with saida.open("a") as f:
        f.write(f"versao={versao}\n")


if __name__ == "__main__":
    main()
