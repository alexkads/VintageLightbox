#!/usr/bin/env python3
"""Sobe o que está em `dist/` para o Supabase Storage e publica o manifesto.

    ./scripts/publicar.py                 # publica o que existir em dist/
    ./scripts/publicar.py --seco          # mostra o que faria
    ./scripts/publicar.py --notas "..."   # o texto que o app mostra na faixa

O que ele faz, em ordem:

1. Lê a versão do `Cargo.toml` do workspace.
2. Varre `dist/` e classifica cada arquivo: o que é **download** (o .dmg, o .deb,
   o instalador do Windows) e o que é **atualização** (o `.app.tar.gz`, o
   `.AppImage`, o `.exe`), com o `.sig` de cada um.
3. Sobe tudo para `versoes/<versao>/` no bucket.
4. Escreve `ultima.json` na raiz do bucket — o arquivo que a página de download e
   o updater do app leem.

🔑 **O `.sig` não é enviado como arquivo: o conteúdo dele entra no manifesto.**
   É assim que o updater espera receber a assinatura, e é o que permite ao app
   conferir o pacote **antes** de instalar. Um `.sig` que não bate faz o app
   recusar a atualização — que é exatamente o ponto.

⚠️ Precisa de duas variáveis no ambiente (ou em ~/.vintagelightbox/publicar.env):

    SUPABASE_URL=https://<projeto>.supabase.co
    SUPABASE_SERVICE_ROLE_KEY=<a service_role>

   A `anon` não serve: o Storage recusa escrita com ela.
"""

import argparse
import json
import mimetypes
import os
import re
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
DIST = RAIZ / "dist"
BUCKET = os.environ.get("VLB_BUCKET", "vintagelightbox")


# ─────────────────────────── o ambiente ───────────────────────────
def carregar_ambiente() -> tuple[str, str]:
    """As credenciais, do ambiente ou do arquivo ao lado da chave de assinatura."""
    env = Path.home() / ".vintagelightbox" / "publicar.env"
    if env.exists():
        for linha in env.read_text().splitlines():
            linha = linha.strip()
            if not linha or linha.startswith("#") or "=" not in linha:
                continue
            chave, valor = linha.split("=", 1)
            os.environ.setdefault(chave.strip(), valor.strip().strip('"').strip("'"))

    url = os.environ.get("SUPABASE_URL", "").rstrip("/")
    key = os.environ.get("SUPABASE_SERVICE_ROLE_KEY", "")
    if not url or not key:
        sys.exit(
            "❌ faltam SUPABASE_URL e/ou SUPABASE_SERVICE_ROLE_KEY.\n"
            f"   Ponha-as no ambiente ou em {env} (que fica fora do repositório).\n"
            "   A service_role é a mesma que o backend usa no Fly:\n"
            "     fly secrets list -a <app>   # confere que existe\n"
        )
    return url, key


def versao_do_workspace() -> str:
    texto = (RAIZ / "Cargo.toml").read_text()
    bloco = texto.split("[workspace.package]", 1)[1]
    m = re.search(r'^version\s*=\s*"([^"]+)"', bloco, re.M)
    if not m:
        sys.exit("❌ não achei a versão em [workspace.package] do Cargo.toml")
    return m.group(1)


# ─────────────────────────── o Storage ───────────────────────────
def requisicao(metodo: str, url: str, key: str, corpo=None, tipo=None):
    req = urllib.request.Request(url, data=corpo, method=metodo)
    req.add_header("Authorization", f"Bearer {key}")
    req.add_header("apikey", key)
    # `x-upsert` porque republicar a mesma versão tem de sobrescrever: sem ele o
    # Storage responde 409 e a segunda tentativa de um lançamento morre no meio.
    req.add_header("x-upsert", "true")
    if tipo:
        req.add_header("Content-Type", tipo)
    return urllib.request.urlopen(req, timeout=600)


def garantir_bucket(base: str, key: str, seco: bool) -> None:
    """Cria o bucket **público** se ele ainda não existir.

    ⚠️ Em `--seco` não toca a rede. Um ensaio que exige credencial válida e
    internet não serve para o que ele existe: conferir, sem risco, o que seria
    publicado.
    """
    if seco:
        print(f"   [seco] conferiria (e criaria, se preciso) o bucket público '{BUCKET}'")
        return

    # 🔑 **Tenta criar primeiro, e trata "já existe" como sucesso.**
    #
    # A ordem natural seria perguntar antes (`GET .../bucket/<nome>`) e criar se
    # não houver — mas o Supabase responde **400** a essa pergunta quando o
    # bucket não existe, com `"statusCode":"404"` **no corpo**. Um código no
    # corpo e outro no cabeçalho: quem confiar no `e.code` erra, e foi o que
    # aconteceu na primeira publicação (7/set/2026).
    #
    # Criar e absorver o conflito não depende de adivinhar qual dos dois códigos
    # o servidor vai usar hoje.
    corpo = json.dumps({"name": BUCKET, "id": BUCKET, "public": True}).encode()
    try:
        requisicao("POST", f"{base}/storage/v1/bucket", key, corpo, "application/json")
        print(f"   bucket '{BUCKET}' criado (público)")
    except urllib.error.HTTPError as e:
        detalhe = e.read().decode(errors="replace")
        if "already exists" in detalhe or "Duplicate" in detalhe:
            print(f"   bucket '{BUCKET}' já existe")
            return
        raise RuntimeError(f"não consegui criar o bucket ({e.code}): {detalhe[:300]}") from e


def subir(base: str, key: str, caminho: Path, destino: str, seco: bool) -> str:
    publico = f"{base}/storage/v1/object/public/{BUCKET}/{destino}"
    tamanho = caminho.stat().st_size
    if seco:
        print(f"   [seco] {destino}  ({tamanho / 1e6:.1f} MB)")
        return publico
    tipo = mimetypes.guess_type(caminho.name)[0] or "application/octet-stream"
    with caminho.open("rb") as f:
        requisicao("POST", f"{base}/storage/v1/object/{BUCKET}/{destino}", key, f.read(), tipo)
    print(f"   ✓ {destino}  ({tamanho / 1e6:.1f} MB)")
    return publico


# ─────────────────── o que dist/ contém, classificado ───────────────────
#
# Cada entrada é (padrão do nome, chave de plataforma do updater, formato).
#
# ⚠️ Os formatos são os quatro que o `cargo-packager-updater` sabe aplicar —
#    `app`, `appimage`, `nsis`, `wix` — e não têm relação com o que se baixa à
#    mão. O `.dmg` e o `.deb` **não** atualizam nada: eles são para a primeira
#    instalação. Quem atualiza no macOS é o `.app.tar.gz`.
#
# 🔑 **A ordem é prioridade.** Quem chegar primeiro a uma plataforma fica com
#    ela — é o que resolve o caso de haver `.exe` e `.msi` no mesmo `dist/`, que
#    disputam `windows-x86_64`. O NSIS vem antes porque é o único dos dois que
#    reabre o app sozinho depois de instalar.
ATUALIZACAO = [
    (r"\.app\.tar\.gz$", ["macos-aarch64", "macos-x86_64"], "app"),
    (r"aarch64\.AppImage$", ["linux-aarch64"], "appimage"),
    (r"\.AppImage$", ["linux-x86_64"], "appimage"),
    (r"-setup\.exe$", ["windows-x86_64"], "nsis"),
    (r"\.msi$", ["windows-x86_64"], "wix"),
]

# O que a página de download oferece, em ordem de apresentação.
DOWNLOADS = [
    (r"universal\.dmg$", "macOS", "Intel e Apple Silicon"),
    (r"aarch64\.dmg$", "macOS", "Apple Silicon"),
    (r"x86_64\.dmg$", "macOS", "Intel"),
    (r"-setup\.exe$", "Windows", "x86-64"),
    (r"\.msi$", "Windows", "x86-64 (MSI)"),
    (r"arm64\.deb$|aarch64\.deb$", "Linux", "ARM64 (.deb)"),
    (r"\.deb$", "Linux", "x86-64 (.deb)"),
    (r"aarch64\.AppImage$", "Linux", "ARM64 (AppImage)"),
    (r"\.AppImage$", "Linux", "x86-64 (AppImage)"),
]


def classificar(arquivos: list[Path]) -> tuple[dict, list]:
    atualizacao: dict[str, dict] = {}
    downloads: list[dict] = []
    for caminho in sorted(arquivos):
        nome = caminho.name
        for padrao, plataformas, formato in ATUALIZACAO:
            if re.search(padrao, nome):
                sig = caminho.with_name(nome + ".sig")
                if not sig.exists():
                    print(f"   ⚠️  {nome} não tem .sig — fica de fora da atualização")
                    break
                for p in plataformas:
                    # Não sobrescreve: a lista acima já é a ordem de preferência.
                    if p in atualizacao:
                        continue
                    atualizacao[p] = {
                        "arquivo": nome,
                        "assinatura": sig.read_text().strip(),
                        "formato": formato,
                    }
                break
        for padrao, sistema, arq in DOWNLOADS:
            if re.search(padrao, nome):
                downloads.append(
                    {
                        "sistema": sistema,
                        "arquitetura": arq,
                        "arquivo": nome,
                        "bytes": caminho.stat().st_size,
                    }
                )
                break
    return atualizacao, downloads


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--seco", action="store_true", help="mostra o que faria, sem subir nada")
    ap.add_argument("--notas", default="", help="o texto que o app mostra ao avisar da versão")
    args = ap.parse_args()

    if not DIST.is_dir():
        sys.exit("❌ não há dist/ — rode ./scripts/empacotar.sh antes")

    base, key = carregar_ambiente()
    versao = versao_do_workspace()

    # Só o que é pacote: o `.app` é uma pasta e sobe como `.app.tar.gz`; os
    # `.sig` viram texto dentro do manifesto e não sobem sozinhos.
    interessa = (".dmg", ".deb", ".AppImage", ".exe", ".msi", ".tar.gz")
    arquivos = [p for p in DIST.rglob("*") if p.is_file() and p.name.endswith(interessa)]
    if not arquivos:
        sys.exit("❌ dist/ não tem pacote nenhum — rode ./scripts/empacotar.sh antes")

    print(f"\n▸ VintageLightbox {versao} → {base}/storage/v1/.../{BUCKET}")
    garantir_bucket(base, key, args.seco)

    atualizacao, downloads = classificar(arquivos)

    print(f"\n▸ subindo {len(arquivos)} arquivo(s) para versoes/{versao}/")
    urls: dict[str, str] = {}
    for caminho in sorted(arquivos):
        urls[caminho.name] = subir(base, key, caminho, f"versoes/{versao}/{caminho.name}", args.seco)

    for p in atualizacao.values():
        p["url"] = urls[p.pop("arquivo")]
    for d in downloads:
        d["url"] = urls[d["arquivo"]]

    manifesto = {
        "versao": versao,
        "publicado_em": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "notas": args.notas,
        # O que o updater do app consome, por plataforma.
        "atualizacao": atualizacao,
        # O que a página de download mostra.
        "downloads": downloads,
    }

    print("\n▸ manifesto")
    print(json.dumps(manifesto, indent=2, ensure_ascii=False))

    corpo = json.dumps(manifesto, ensure_ascii=False, indent=2).encode()
    for destino in (f"versoes/{versao}/lancamento.json", "ultima.json"):
        if args.seco:
            print(f"   [seco] escreveria {destino}")
            continue
        requisicao(
            "POST", f"{base}/storage/v1/object/{BUCKET}/{destino}", key, corpo, "application/json"
        )
        print(f"   ✓ {destino}")

    if atualizacao:
        print(f"\n✅ {versao} publicada — {len(atualizacao)} plataforma(s) recebem atualização")
    else:
        print(
            f"\n⚠️  {versao} publicada **sem atualização automática**: nenhum pacote tinha .sig.\n"
            "   Gere a chave e reempacote — ./scripts/empacotar.sh explica."
        )
    print("   página: https://recordarfotos.com.br/vintageLightbox")


if __name__ == "__main__":
    # 🔑 Erro de rede e erro do Storage viram uma linha, e não um traceback de
    #    trinta. O que quem publica precisa saber é "não subiu, e por quê" — o
    #    resto é ruído entre ele e a resposta.
    try:
        main()
    except urllib.error.HTTPError as e:
        corpo = e.read().decode(errors="replace")[:400]
        sys.exit(f"\n❌ o Storage recusou ({e.code}): {corpo}")
    except urllib.error.URLError as e:
        sys.exit(f"\n❌ não consegui falar com o Supabase: {e.reason}")
