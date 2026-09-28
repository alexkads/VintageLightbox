#!/usr/bin/env python3
"""Baixa os ícones e as fontes que o template do app pede.

O template do `ui-gpui` segue o `ui.shadcn.com/create`: uma biblioteca de
ícones e uma ou duas famílias de letra. Este script traz os arquivos de cada
escolha para dentro de `crates/ui-gpui/`, onde o `build.rs` os embute.

    --icones <biblioteca>   tabler | hugeicons | phosphor | remixicon
    --fonte <slug>          uma das 26 do /create (pode repetir)
    --listar                o que cada biblioteca cobre, e o que fica no lucide

🔑 **Os ícones são função do `template/icones.json`.** A pasta
   `icones-<biblioteca>/` é apagada e refeita a cada rodada: o que o json não
   diz, não fica. Um ícone sem equivalente fica de fora de propósito, e o
   `build.rs` usa o lucide de `icones/` no lugar — melhor um lucide no meio de
   outra biblioteca que um desenho que diz outra coisa.

🔑 **As fontes vêm do Google Fonts, a mesma origem do `@fontsource` que o
   shadcn usa.** Pedimos o CSS sem navegador no user-agent de propósito: é assim
   que o Google devolve `.ttf` inteiro, um por peso, que é o que o GPUI carrega.
   Com um user-agent de navegador viriam `.woff2` recortados por alfabeto.

⚠️ **O Iconify recusa o user-agent do urllib (403)**, então as chamadas a ele
   vão com o do curl. O Google Fonts aceita o padrão, e é com ele que vão.

⚠️ **Os ícones vêm em lote, pelo `.json` da coleção, e não um `.svg` por vez.**
   Cento e tantas requisições seguidas por biblioteca esbarram no limite de
   taxa do Iconify (429, e depois bloqueio do Cloudflare por alguns minutos).
   O `<svg>` montado aqui é o mesmo que o endpoint de SVG devolveria.

Só biblioteca padrão: roda em qualquer máquina de desenvolvimento, sem pip.
"""

import argparse
import json
import pathlib
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from datetime import date

RAIZ = pathlib.Path(__file__).resolve().parent.parent
APP = RAIZ / "crates" / "ui-gpui"
ICONES_JSON = APP / "template" / "icones.json"
ICONES_LUCIDE = APP / "icones"

# O nome do /create → o prefixo do Iconify.
PREFIXOS = {
    "tabler": "tabler",
    "hugeicons": "hugeicons",
    "phosphor": "ph",
    "remixicon": "ri",
}

# O slug do /create → a família no Google Fonts. É a mesma tabela de
# `src/tema/fontes.rs`; as duas têm de andar juntas.
FAMILIAS = {
    "inter": "Inter",
    "noto-sans": "Noto Sans",
    "nunito-sans": "Nunito Sans",
    "figtree": "Figtree",
    "roboto": "Roboto",
    "raleway": "Raleway",
    "dm-sans": "DM Sans",
    "public-sans": "Public Sans",
    "outfit": "Outfit",
    "jetbrains-mono": "JetBrains Mono",
    "geist": "Geist",
    "geist-mono": "Geist Mono",
    "lora": "Lora",
    "merriweather": "Merriweather",
    "playfair-display": "Playfair Display",
    "noto-serif": "Noto Serif",
    "roboto-slab": "Roboto Slab",
    "oxanium": "Oxanium",
    "manrope": "Manrope",
    "space-grotesk": "Space Grotesk",
    "montserrat": "Montserrat",
    "ibm-plex-sans": "IBM Plex Sans",
    "source-sans-3": "Source Sans 3",
    "instrument-sans": "Instrument Sans",
    "eb-garamond": "EB Garamond",
    "instrument-serif": "Instrument Serif",
}

# Os pesos que o tema usa: corpo, médio, seminegrito e negrito.
PESOS = (400, 500, 600, 700)

UA_ICONIFY = {"User-Agent": "curl/8.7.1"}

# Ícones por pedido ao Iconify: a URL fica curta e o lote cabe numa resposta.
LOTE = 60


def baixar(url, cabecalhos=None, tentativas=5):
    """Os bytes de `url`, ou `None` se o servidor disser que não existe.

    Um 429 espera e tenta de novo: o Iconify limita a taxa por endereço, e uma
    rodada logo depois da outra esbarra nele.
    """
    pedido = urllib.request.Request(url, headers=cabecalhos or {})
    for tentativa in range(tentativas):
        try:
            with urllib.request.urlopen(pedido, timeout=60) as resposta:
                return resposta.read()
        except urllib.error.HTTPError as erro:
            if erro.code in (400, 404):
                return None
            if erro.code != 429 or tentativa == tentativas - 1:
                raise
            espera = int(erro.headers.get("Retry-After") or 0) or 5 * 2**tentativa
            print(f"   ⏳ limite de taxa em {urllib.parse.urlsplit(url).netloc}; esperando {espera}s")
            time.sleep(espera)
    return None


def ler_mapa():
    return json.loads(ICONES_JSON.read_text(encoding="utf-8"))


# ── ícones ────────────────────────────────────────────────────────────────


def licenca_da_colecao(prefixo):
    """O que o Iconify sabe da coleção: nome, autor, licença, versão."""
    dados = baixar(
        f"https://api.iconify.design/collections?prefixes={prefixo}", UA_ICONIFY
    )
    return json.loads(dados)[prefixo]


def texto_da_licenca(url):
    """O texto integral, quando a URL é de um arquivo no GitHub.

    O Iconify aponta para a página (`/blob/`); o texto cru está no
    `raw.githubusercontent.com`. Sem URL, ou se ela não for do GitHub, não há
    o que baixar — o cabeçalho do LICENSE diz então onde procurar.
    """
    if not url:
        return None
    cru = re.sub(
        r"^https://github\.com/([^/]+)/([^/]+)/blob/",
        r"https://raw.githubusercontent.com/\1/\2/",
        url,
    )
    if cru == url:
        return None
    dados = baixar(cru)
    return dados.decode("utf-8") if dados else None


def svg_valido(texto):
    """Confere que é XML e que tem viewBox: sem ele o GPUI não sabe escalar."""
    try:
        raiz = ET.fromstring(texto)
    except ET.ParseError:
        return False
    return raiz.tag.endswith("svg") and "viewBox" in raiz.attrib


def svgs_da_colecao(prefixo, pedidos):
    """{chave: svg} para os ícones pedidos, montados como o Iconify monta.

    🔑 **Um pedido por lote, e não um por ícone.** O `/<prefixo>/<nome>.svg`
    custa uma requisição por arquivo — 150 por biblioteca — e o Iconify corta
    com 429 no meio da segunda. O `/<prefixo>.json?icons=` devolve o mesmo
    desenho em lote, e o `<svg>` em volta é o que o endpoint de SVG escreveria:
    `width="1em" height="1em"` e o viewBox do ícone.
    """
    nomes = sorted(set(pedidos.values()))
    desenhos = {}
    for inicio in range(0, len(nomes), LOTE):
        lote = nomes[inicio:inicio + LOTE]
        dados = baixar(
            f"https://api.iconify.design/{prefixo}.json?icons=" + ",".join(lote), UA_ICONIFY
        )
        colecao = json.loads(dados) if dados else {}
        icones = colecao.get("icons", {})
        # Um nome que é apelido vem em `aliases`, apontando o desenho de verdade.
        for apelido, alvo in colecao.get("aliases", {}).items():
            if alvo.get("parent") in icones and len(alvo) == 1:
                icones[apelido] = icones[alvo["parent"]]
        for nome in lote:
            icone = icones.get(nome)
            if icone is None:
                continue
            esquerda = icone.get("left", colecao.get("left", 0))
            topo = icone.get("top", colecao.get("top", 0))
            largura = icone.get("width", colecao.get("width", 16))
            altura = icone.get("height", colecao.get("height", 16))
            desenhos[nome] = (
                '<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" '
                f'viewBox="{esquerda} {topo} {largura} {altura}">{icone["body"]}</svg>'
            )
    return {chave: desenhos.get(nome) for chave, nome in pedidos.items()}


def baixar_icones(biblioteca):
    prefixo = PREFIXOS[biblioteca]
    mapa = ler_mapa()
    pedidos = {chave: nomes[biblioteca] for chave, nomes in mapa.items() if biblioteca in nomes}
    pasta = APP / f"icones-{biblioteca}"

    # Baixa tudo antes de mexer na pasta: uma rede que cai no meio não deixa
    # a biblioteca pela metade no disco.
    print(f"⬇️  {biblioteca}: {len(pedidos)} ícones de api.iconify.design/{prefixo}")
    svgs = svgs_da_colecao(prefixo, pedidos)

    # A pasta é função do json: o que sobrou de uma rodada anterior sai.
    pasta.mkdir(parents=True, exist_ok=True)
    antigos = list(pasta.glob("*.svg"))
    for arquivo in antigos:
        arquivo.unlink()
    print(f"🧹 {biblioteca}: {len(antigos)} SVGs antigos apagados")

    falhas = []
    for chave, svg in sorted(svgs.items()):
        if svg is None:
            falhas.append(f"{chave} → {prefixo}:{pedidos[chave]} (não existe na coleção)")
            continue
        # O Iconify entrega em `1em`, que é medida de CSS; o GPUI quer número.
        # O viewBox fica como veio — é ele que diz a proporção.
        svg = svg.replace('width="1em" height="1em"', 'width="24" height="24"', 1)
        if not svg_valido(svg):
            falhas.append(f"{chave} → {prefixo}:{pedidos[chave]} (SVG sem viewBox ou inválido)")
            continue
        (pasta / f"{chave}.svg").write_text(svg, encoding="utf-8")

    colecao = licenca_da_colecao(prefixo)
    licenca = colecao.get("license", {})
    autor = colecao.get("author", {})
    texto = texto_da_licenca(licenca.get("url"))
    cabecalho = [
        f"{colecao.get('name', biblioteca)}"
        + (f" {colecao['version']}" if colecao.get("version") else ""),
        f"Autor: {autor.get('name', '?')}" + (f" — {autor['url']}" if autor.get("url") else ""),
        f"Licença: {licenca.get('title', '?')} ({licenca.get('spdx', '?')})"
        + (f" — {licenca['url']}" if licenca.get("url") else ""),
        f"Origem: https://api.iconify.design/{prefixo}.json (o mesmo desenho de "
        f"/{prefixo}/<nome>.svg), baixados em {date.today().isoformat()} por scripts/baixar-do-template.py",
        "O nome de cada arquivo é o do ícone lucide que ele substitui; o nome",
        "original na coleção está em crates/ui-gpui/template/icones.json.",
    ]
    if texto is None:
        cabecalho.append(
            "O texto integral da licença não foi baixado: a coleção não aponta um arquivo"
            " no GitHub. Ver a página do autor acima."
        )
    corpo = "\n".join(cabecalho) + "\n"
    if texto:
        corpo += "\n---\n\n" + texto
    (pasta / "LICENSE").write_text(corpo, encoding="utf-8")

    gravados = len(list(pasta.glob("*.svg")))
    print(f"✅ {biblioteca}: {gravados} SVGs em {pasta.relative_to(RAIZ)}, licença {licenca.get('spdx', '?')}")
    return falhas


# ── fontes ────────────────────────────────────────────────────────────────


def faces_do_css(css):
    """[(peso, url)] de cada `@font-face` em `.ttf` do CSS do Google."""
    faces = []
    for bloco in re.findall(r"@font-face\s*{([^}]*)}", css):
        peso = re.search(r"font-weight:\s*(\d+)", bloco)
        url = re.search(r"src:\s*url\(([^)]+)\)\s*format\('truetype'\)", bloco)
        if peso and url:
            faces.append((int(peso.group(1)), url.group(1)))
    return faces


def css_da_familia(familia, pesos):
    consulta = urllib.parse.quote(familia) + ":wght@" + ";".join(str(p) for p in pesos)
    dados = baixar(f"https://fonts.googleapis.com/css2?family={consulta}")
    return dados.decode("utf-8") if dados else None


def baixar_fonte(slug):
    familia = FAMILIAS[slug]
    pasta = APP / "fontes" / slug
    pasta.mkdir(parents=True, exist_ok=True)
    print(f"🔤 {slug}: família \"{familia}\"")

    # Pedido combinado primeiro. Se o Google recusar por causa de um peso que a
    # família não tem, pede um a um e fica com os que existirem.
    css = css_da_familia(familia, PESOS)
    faces = faces_do_css(css) if css else []
    if not faces:
        for peso in PESOS:
            css = css_da_familia(familia, [peso])
            if css:
                faces += faces_do_css(css)
    if not faces:
        return [f"{slug}: o Google Fonts não devolveu nenhum .ttf para \"{familia}\""]

    for antigo in pasta.glob("*.ttf"):
        antigo.unlink()
    vistos = set()
    for peso, url in sorted(faces):
        if peso in vistos:
            continue
        vistos.add(peso)
        destino = pasta / f"{slug}-{peso}.ttf"
        destino.write_bytes(baixar(url))
        print(f"   {destino.name} ({destino.stat().st_size // 1024} KB)")
    faltam = [p for p in PESOS if p not in vistos]
    if faltam:
        print(f"   a família não tem os pesos {', '.join(map(str, faltam))}")

    # A licença mora no repositório google/fonts, numa pasta com o nome da
    # família sem espaços. Quase todas são OFL; umas poucas, Apache.
    pasta_gf = familia.lower().replace(" ", "")
    licenca = None
    for url in (
        f"https://raw.githubusercontent.com/google/fonts/main/ofl/{pasta_gf}/OFL.txt",
        f"https://raw.githubusercontent.com/google/fonts/main/apache/{pasta_gf}/LICENSE.txt",
    ):
        licenca = baixar(url)
        if licenca:
            print(f"   licença de {url}")
            break
    if licenca is None:
        licenca = (
            f"A licença de \"{familia}\" não foi achada em google/fonts (ofl/ nem apache/).\n"
            f"Conferir em https://fonts.google.com/specimen/{familia.replace(' ', '+')}/license\n"
        ).encode("utf-8")
        print("   ⚠️  licença não encontrada; gravado um aviso no lugar")
    (pasta / "OFL.txt").write_bytes(licenca)
    return []


# ── listagem ──────────────────────────────────────────────────────────────


def listar():
    mapa = ler_mapa()
    lucide = sorted(p.stem for p in ICONES_LUCIDE.glob("*.svg"))
    sem_entrada = [nome for nome in lucide if nome not in mapa]
    for biblioteca, prefixo in PREFIXOS.items():
        cobre = [nome for nome in lucide if biblioteca in mapa.get(nome, {})]
        fica = [nome for nome in lucide if nome not in cobre]
        print(f"{biblioteca} ({prefixo}): {len(cobre)} de {len(lucide)} com equivalente")
        print(f"   ficam no lucide: {', '.join(fica) or 'nenhum'}")
    if sem_entrada:
        print(f"⚠️  ícones de icones/ sem entrada no json: {', '.join(sem_entrada)}")


def main():
    parser = argparse.ArgumentParser(
        description="Baixa os ícones e as fontes do template do ui-gpui."
    )
    parser.add_argument("--icones", choices=sorted(PREFIXOS), action="append", default=[],
                        help="biblioteca de ícones a baixar (pode repetir)")
    parser.add_argument("--fonte", choices=sorted(FAMILIAS), action="append", default=[],
                        metavar="SLUG", help="fonte do /create a baixar (pode repetir)")
    parser.add_argument("--listar", action="store_true",
                        help="mostra a cobertura de cada biblioteca")
    args = parser.parse_args()
    if not (args.icones or args.fonte or args.listar):
        parser.print_help()
        return 2

    if args.listar:
        listar()
    falhas = []
    for biblioteca in args.icones:
        falhas += baixar_icones(biblioteca)
    for slug in args.fonte:
        falhas += baixar_fonte(slug)
    if falhas:
        print("❌ falhas:")
        for falha in falhas:
            print(f"   {falha}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
