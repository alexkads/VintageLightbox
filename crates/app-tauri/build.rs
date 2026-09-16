fn main() {
    // 🔒 Declarar os comandos aqui é o que os torna **negados por padrão**: sem a
    // lista, o Tauri libera todo comando do app para qualquer página carregada.
    // Com ela, cada um só responde a quem a capacidade nomear (§6, regra 1).
    let comandos = tauri_build::AppManifest::new().commands(&[
        "escolher_raw",
        "ler_raw",
        "converter_raw",
        "cartoes_montados",
        "escolher_origem",
        "listar_origem",
        "ler_da_origem",
        "pasta_de_saida",
        "escolher_pasta",
        "esquecer_pasta",
        "nomes_na_pasta",
        "gravar_na_pasta",
        "abrir_tela_do_cliente",
        "fechar_tela_do_cliente",
        "registrar_no_terminal",
        "ha_sessao",
        "entrar",
        "sair",
        "chamar_api",
        "guardar_envio",
        "fila_de_envios",
        "tentar_envios_agora",
        "esquecer_envio",
        "envios_recusados",
        "pendentes_na_pagina",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(comandos))
        .expect("o tauri-build falhou");
}
