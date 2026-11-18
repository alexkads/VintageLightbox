# gpui-pre-wgpu 0.3.7 — captura da janela (#35)

Snapshot de crates.io, Apache-2.0 (LICENSE-APACHE), zed@1a28cff.

`WgpuRenderer::render_to_image` desenha a cena numa textura fora da tela com o
mesmo core e atlas da janela. Lê as linhas respeitando o padding de WGPU e
converte BGRA para RGBA. Usa as mesmas primitivas do readback headless existente.
`image` deixa de ser dependência opcional, pois a captura também existe no app.

Os backends X11 e Wayland delegam a este método. Não captura o desktop.
