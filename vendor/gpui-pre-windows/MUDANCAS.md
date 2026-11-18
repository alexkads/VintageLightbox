# gpui-pre-windows 0.3.7 — captura da janela (#35)

Snapshot de crates.io, Apache-2.0 (LICENSE-APACHE), zed@1a28cff.

Retirado o gate test-support dos métodos `render_to_image` em `window.rs` e
`directx_renderer.rs`. O readback DirectX existente passa a atender a CLI no
app normal. Nenhuma alteração no algoritmo de captura.

Reaplicar esses dois gates ao atualizar o snapshot e validar no Windows nativo.
