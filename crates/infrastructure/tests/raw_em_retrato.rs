//! A foto em retrato sai de pé, e não embaralhada.
//!
//! Precisa das amostras em `~/Documents/RAWSample` (ou `VLB_RAWS`), por isso é
//! `#[ignore]`: `cargo test -p infrastructure --test raw_em_retrato -- --ignored`.

use rsraw::RawImage;

#[test]
#[ignore]
fn as_dimensoes_seguem_a_rotacao_da_camera() {
    let pasta = std::env::var("VLB_RAWS")
        .unwrap_or_else(|_| format!("{}/Documents/RAWSample", std::env::var("HOME").unwrap()));
    let mut conferidas = 0;
    let mut em_retrato = 0;
    for entrada in std::fs::read_dir(&pasta).expect("sem a pasta de amostras") {
        let caminho = entrada.unwrap().path();
        let texto = caminho.to_string_lossy().to_string();
        if !infrastructure::raw_processing::is_raw_file(&texto) {
            continue;
        }
        let bytes = std::fs::read(&caminho).unwrap();
        let Ok(raw) = RawImage::open(&bytes) else {
            continue;
        };
        let tamanhos = &AsRef::<rsraw_sys::libraw_data_t>::as_ref(&raw).sizes;
        let girada = tamanhos.flip & 4 != 0;
        let (largura, altura) = (raw.width(), raw.height());
        drop(raw);

        let imagem = infrastructure::raw_processing::load_raw_from_bytes(&bytes)
            .unwrap_or_else(|e| panic!("{texto}: {e}"));
        let esperado = if girada {
            (altura, largura)
        } else {
            (largura, altura)
        };
        assert_eq!((imagem.width(), imagem.height()), esperado, "{texto}");
        conferidas += 1;
        if girada {
            em_retrato += 1;
        }
    }
    eprintln!("{conferidas} RAWs conferidos, {em_retrato} em retrato");
    assert!(conferidas > 0);
}
