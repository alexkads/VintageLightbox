#[test]
#[ignore]
fn o_dng_com_perdas_abre_pela_reserva() {
    let caminho = "/Users/alexkads/Documents/FotosParaSite/_CSF7953.dng";
    let img = infrastructure::raw_processing::load_raw_as_dynamic_image(caminho)
        .expect("o DNG com perdas tinha de abrir pela LibRaw do sistema");
    assert!(
        img.width() > 1000,
        "veio prévia em vez da foto: {}",
        img.width()
    );
}
