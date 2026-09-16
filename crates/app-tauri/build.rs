fn main() {
    // 🔒 Declarar os comandos aqui é o que os torna **negados por padrão**: sem a
    // lista, o Tauri libera todo comando do app para qualquer página carregada.
    // Com ela, cada um só responde a quem a capacidade nomear (§6, regra 1).
    let comandos = tauri_build::AppManifest::new().commands(&["escolher_raw", "ler_raw"]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(comandos))
        .expect("o tauri-build falhou");
}
