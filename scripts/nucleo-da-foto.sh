# Onde está o núcleo da foto que este app consome — para os scripts que
# entregam os `.wasm` do motor ao site (`source scripts/nucleo-da-foto.sh`).
#
# 📸 Os crates do motor e os scripts que os compilam para o navegador moram no
# núcleo desde 18/out/2026 (github.com/alexkads/nucleo-da-foto). Daqui sai só a
# escolha da cópia:
#
#   - `NUCLEO_DA_FOTO=<pasta>`: um checkout local, para desenvolver junto
#     (em geral `../../PhotoKads/nucleo-da-foto`, o mesmo do `[patch]` local);
#   - sem ela, um clone raso **na tag do `Cargo.toml`** em
#     `target/nucleo-da-foto/<tag>`. 🔑 O site recebe o mesmo motor que o app
#     compila — duas tags seriam duas revelações diferentes da mesma foto.

nucleo_da_foto() {
    if [[ -n "${NUCLEO_DA_FOTO:-}" ]]; then
        (cd "$NUCLEO_DA_FOTO" && pwd)
        return
    fi
    local tag
    tag="$(sed -n 's/^revelacao-core = .*tag = "\([^"]*\)".*/\1/p' Cargo.toml)"
    if [[ -z "$tag" ]]; then
        echo "não achei a tag do núcleo no Cargo.toml" >&2
        return 1
    fi
    local destino="target/nucleo-da-foto/$tag"
    if [[ ! -d "$destino/.git" ]]; then
        echo "→ núcleo da foto $tag (clone raso em $destino)" >&2
        rm -rf "$destino"
        git clone -q --depth 1 --branch "$tag" \
            https://github.com/alexkads/nucleo-da-foto "$destino" >&2
    fi
    (cd "$destino" && pwd)
}

# Repassa ao script do núcleo, com o frontend em caminho absoluto: ele roda de
# dentro do núcleo, onde o `../recordarfotos-e-commerce` padrão não existe.
construir_pelo_nucleo() {
    local script="$1"
    shift
    local opcoes=()
    if [[ "${1:-}" == "--sem-pilha" ]]; then
        opcoes+=(--sem-pilha)
        shift
    fi
    local frontend="${1:-${RECORDARFOTOS_FRONTEND:-../recordarfotos-e-commerce/frontend}}"
    if [[ ! -d "$frontend/public" ]]; then
        echo "frontend não encontrado em $frontend (passe o caminho ou RECORDARFOTOS_FRONTEND)" >&2
        exit 1
    fi
    local nucleo
    nucleo="$(nucleo_da_foto)"
    exec "$nucleo/scripts/$script" ${opcoes[@]+"${opcoes[@]}"} "$(cd "$frontend" && pwd)"
}
