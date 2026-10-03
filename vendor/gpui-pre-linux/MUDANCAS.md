# gpui-pre-linux 0.3.7 — vendorizado

Origem: <https://crates.io/crates/gpui-pre-linux/0.3.7> (o `gpui_linux` do Zed,
snapshot `zed@1a28cff`), licença Apache-2.0 (ver `LICENSE-APACHE`). Trazido
para cá em 2026-10-03 porque a Tela do cliente nascia no monitor errado no
Fedora (GNOME/Wayland).

## O que mudou em relação ao original

Só `src/linux/wayland/window.rs`:

1. `WaylandWindowState` ganhou `fullscreen_output`, o `wl_output` do
   `WindowOptions::display_id`. No original o `display_id` virava
   `target_output` e só servia a janelas *layer-shell*; numa janela comum
   (`xdg_toplevel`) ele era descartado.
2. `toggle_fullscreen` passa esse monitor a `set_fullscreen` **enquanto a
   janela ainda não apareceu** (`display` vazio). É o caso de
   `WindowBounds::Fullscreen` na abertura: o GPUI chama `toggle_fullscreen`
   logo depois de criar a janela. Depois que ela entrou num monitor, a tela
   cheia é a de sempre (`None`): fica onde o operador a pôs.

🔑 O xdg-shell não deixa o app posicionar janela; `set_fullscreen(output)` é o
único pedido de lugar que o GNOME atende. Por isso a Tela do cliente, no
Wayland, nasce em tela cheia no monitor escolhido
(`crates/ui-gpui/src/cliente.rs`, `ao_nascer`).

## Quando sair

Quando o GPUI de cima passar o `target_output` ao `set_fullscreen`. Para
atualizar até lá: copiar a versão nova do crates.io, reaplicar os itens 1–2
(procurar `VintageLightbox` no arquivo) e conferir no Fedora com dois monitores.
