# A toolchain Linux, para gerar o .deb e o AppImage **daqui, do Mac**.
#
# 🔑 Por que contêiner e não cross-compilação: o app tem C e C++ dentro —
#    LibRaw (`rsraw-sys`, ~200 .cpp), SQLite embutido (`rusqlite` bundled),
#    OpenSSL (`native-tls`) — e o GPUI no Linux linka X11, Wayland, xkbcommon,
#    fontconfig e Vulkan. Não é o **compilador** que falta no macOS (zig e mingw
#    dariam conta): é o **sysroot** com essas bibliotecas e seus headers.
#    Montá-lo à mão é o caminho longo para o mesmo lugar; um contêiner **é** um
#    Linux, e compila nativo.
#
# ⚠️ Ubuntu 22.04 de propósito, e não a mais nova: o binário passa a exigir a
#    glibc da imagem em que foi compilado. 22.04 (glibc 2.35) roda em Ubuntu
#    22.04+, Debian 12+, Fedora 36+. Subir para 24.04 quebraria o 22.04, que
#    ainda é LTS.
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV CARGO_TERM_COLOR=always

# ── 1. O mínimo para ter Rust ────────────────────────────────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential pkg-config curl ca-certificates git file \
    && rm -rf /var/lib/apt/lists/*

# ── 2. Rust e o empacotador ──────────────────────────────────────────────────
#
# 🔑 **Esta é a camada cara, e ela vem antes da lista de bibliotecas de
#    propósito.** A lista abaixo é a que muda quando falta alguma coisa — e com
#    a ordem invertida, acrescentar um `-dev` recompilava o cargo-packager
#    inteiro, que sob emulação x86_64 num Mac ARM custa dezenas de minutos.
#
# A mesma versão do rustc que o Mac usa — versão diferente é compilador
# diferente, e o objetivo aqui é o mesmo binário em outro sistema.
ARG RUST_VERSION=1.98.0
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain "${RUST_VERSION}" --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

ARG PACKAGER_VERSION=0.11.8
RUN cargo install cargo-packager --locked --version "${PACKAGER_VERSION}"

# ── 3. O que o app precisa para compilar e para abrir ────────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
      # 🚨 bindgen. O `rsraw-sys` gera as ligações do LibRaw com o bindgen, que
      #    carrega a `libclang.so` em tempo de execução — sem ela o build morre
      #    com "Unable to find libclang", que não parece falta de pacote.
      clang libclang-dev \
      # o C++ do LibRaw
      libstdc++-12-dev \
      # o que o GPUI abre no Linux
      libx11-dev libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev \
      libwayland-dev wayland-protocols libxcursor-dev libxrandr-dev libxi-dev \
      libfontconfig1-dev libfreetype6-dev \
      # som (o `scap`/`gpui_media` puxa), TLS e Vulkan
      libasound2-dev libssl-dev libvulkan-dev \
      # o chaveiro do Linux que o `keyring` usa (Secret Service via D-Bus)
      libdbus-1-dev libsecret-1-dev \
      # o AppImage precisa montar um squashfs e o linuxdeploy baixa por FUSE
      fuse libfuse2 desktop-file-utils zsync \
    && rm -rf /var/lib/apt/lists/*

# ⚠️ O `linuxdeploy` monta o próprio AppImage por FUSE, e um contêiner comum não
#    tem `/dev/fuse`. Esta variável manda extraí-lo em vez de montá-lo — é o modo
#    que os projetos que empacotam AppImage em CI usam, e sem ela a etapa falha
#    com "fuse: device not found" já no fim de tudo.
ENV APPIMAGE_EXTRACT_AND_RUN=1

WORKDIR /projeto
